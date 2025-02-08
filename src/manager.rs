use std::sync::Arc;

use ahash::{AHashMap, AHashSet};
use chrono::Local;
use faststr::FastStr;
use futures::future;
use tokio::task::JoinHandle;

use crate::{exector::Executor, middlerware::Middlerware, tracing_info::TracingInfoManager};

pub struct Manager {
    // base field
    timeout_ms: u64,
    adjacency_list: AHashMap<&'static str, Vec<&'static str>>,
    rev_adjacency_list: AHashMap<&'static str, Vec<&'static str>>,
    exectors: AHashMap<&'static str, Box<dyn Executor>>,

    // for extension feild
    middlerware: Option<Middlerware>,

    // inner field
    _tracing: TracingInfoManager,
}

impl Manager {
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            timeout_ms,
            adjacency_list: AHashMap::new(),
            rev_adjacency_list: AHashMap::new(),
            exectors: AHashMap::new(),
            middlerware: None,
            _tracing: TracingInfoManager::new(),
        }
    }

    pub fn set_middlerware(&mut self, middlerware: Middlerware) {
        self.middlerware = Some(middlerware);
    }

    pub fn add_exector(&mut self, exector: Box<dyn Executor>) {
        if self.exectors.contains_key(exector.name()) {
            panic!("exector name repeat: {}", exector.name());
        }

        self.exectors.insert(exector.name(), exector);
    }

    pub fn add_exectors(&mut self, exectors: Vec<Box<dyn Executor>>) {
        for exector in exectors {
            self.add_exector(exector);
        }
    }

    pub fn add_edge(&mut self, from: &'static str, to: &'static str) {
        if self.adjacency_list.contains_key(from) {
            if self.adjacency_list[from].contains(&to) {
                return;
            }
        }

        self.adjacency_list
            .entry(from)
            .or_insert_with(Vec::new)
            .push(to);
        self.rev_adjacency_list
            .entry(to)
            .or_insert_with(Vec::new)
            .push(from);
    }

    pub fn add_edges(&mut self, from: &'static str, to_list: Vec<&'static str>) {
        for to in to_list {
            self.add_edge(from, to);
        }
    }

    pub fn add_dep(&mut self, name: &'static str, dep: &'static str) {
        self.add_edge(dep, name);
    }

    pub fn add_deps(&mut self, name: &'static str, deps: Vec<&'static str>) {
        for dep in deps {
            self.add_dep(name, dep);
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        tokio::time::timeout(
            std::time::Duration::from_millis(self.timeout_ms),
            self.run_inner(),
        )
        .await
        .map_or_else(
            |err| {
                tracing::error!(
                    "run timeout!!!, time limit is {} ms, err is {:?}",
                    self.timeout_ms,
                    err
                );
                tracing::error!("exector tracing info: {}", self._tracing);
                Err(err.into())
            },
            |res| {
                tracing::info!("exector tracing info: {}", self._tracing);
                res
            },
        )
    }

    async fn run_inner(&mut self) -> anyhow::Result<()> {
        let start_exectors = self.pre_check_and_find_start_nodes()?;

        let mut handles: Vec<_> = start_exectors.iter().map(|exector_name| {
            let exector = self.exectors.remove(exector_name).unwrap();
            self._tracing.start(exector.name());
            self.build_handle(exector)
        }).collect();

        let mut all_ready_exector_names = AHashSet::new();
        while !handles.is_empty() {
            let (ready_handle, _, remain_handles) = future::select_all(handles).await;
            if ready_handle.is_err() {
                // is not exector response error, is select all error, so panic
                panic!("join handle error: {:?}", ready_handle);
            }

            let ready_exector_name = match ready_handle.unwrap() {
                Ok(name) => name,
                Err((name, err)) => {
                    tracing::error!("exector {} error: {:?}", name, err);
                    name
                }
            };
            all_ready_exector_names.insert(ready_exector_name);
            self._tracing.done(ready_exector_name);

            let mut new_handles = remain_handles;
            if let Some(next_exector_names) =  self.adjacency_list.get(ready_exector_name) {
                for next_exector_name in next_exector_names {
                    if let Some(next_exector_deps) = self.rev_adjacency_list.get(next_exector_name) {
                        if next_exector_deps.iter().all(|dep| all_ready_exector_names.contains(dep)) {
                            let next_exector = self.exectors.remove(next_exector_name).unwrap();
                            self._tracing.start(next_exector.name());
                            new_handles.push(self.build_handle(next_exector));
                        }
                    }
                }
            }

            handles = new_handles;
        }

        Ok(())

    }

    fn pre_check_and_find_start_nodes(&self) -> anyhow::Result<Vec<&'static str>> {
        let start_nodes = self.find_start_nodes();
        if start_nodes.is_empty() {
            return Err(anyhow::anyhow!("no start nodes, maybe has cycle"));
        }

        // check cycle
        let mut visited = AHashMap::new();
        let mut stack = Vec::new();
        for start_node in start_nodes.iter() {
            stack.push(start_node);
            visited.insert(start_node, false);
        }

        while let Some(node) = stack.pop() {
            if let Some(neighbors) = self.adjacency_list.get(node) {
                for neighbor in neighbors.iter() {
                    if visited.get(neighbor).cloned().unwrap_or(false) {
                        return Err(anyhow::anyhow!(
                            "find cycle, please check {} to {}",
                            node,
                            neighbor
                        ));
                    } else {
                        stack.push(neighbor);
                        visited.insert(neighbor, false);
                    }
                }
            }

            visited.insert(node, true);
        }

        Ok(start_nodes)
    }

    fn find_start_nodes(&self) -> Vec<&'static str> {
        let mut start_nodes = Vec::new();
        for (name, _) in self.exectors.iter() {
            if !self.rev_adjacency_list.contains_key(name) {
                start_nodes.push(name.clone());
            }
        }

        start_nodes
    }

    fn build_handle(&self, exector: Box<dyn Executor>) -> JoinHandle<Result<&'static str, (&'static str, anyhow::Error)>> {
        let name = exector.name();

        if let Some(middlerware) = &self.middlerware {
            let wrap_exector = middlerware(exector);
            tokio::spawn(async move {
                let res = wrap_exector.await;
                if let Err(err) = res {
                    Err((name, err))
                } else {
                    Ok(name)
                }
            })
        } else {
            tokio::spawn(async move {
                let res = exector.execute().await;
                if let Err(err) = res {
                    Err((name, err))
                } else {
                    Ok(name)
                }
            })
        }
    }
}
