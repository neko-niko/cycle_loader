use std::fmt::Display;

use ahash::AHashMap;
use chrono::Local;
use faststr::FastStr;

pub(crate) enum Status {
    NotStarted,
    Doing,
    Done,
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::NotStarted => write!(f, "NotStarted"),
            Status::Doing => write!(f, "Doing"),
            Status::Done => write!(f, "Done"),
        }
    }
}

pub(crate) struct TracingInfo {
    pub(crate) status: Status,
    pub(crate) start_time: i64,
    pub(crate) end_time: i64,
}

impl Display for TracingInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            Status::Doing => {
                let now = Local::now().timestamp_micros();
                write!(
                    f,
                    "status: {}, start_time: {}, now: {}",
                    self.status, self.start_time, now
                )
            }
            _ => write!(
                f,
                "status: {}, start_time: {}, end_time: {}",
                self.status, self.start_time, self.end_time
            ),
        }
    }
}

impl TracingInfo {
    pub(crate) fn new() -> Self {
        Self {
            status: Status::NotStarted,
            start_time: 0,
            end_time: 0,
        }
    }

    pub(crate) fn start(&mut self) {
        match self.status {
            Status::NotStarted => {
                self.status = Status::Doing;
                self.start_time = Local::now().timestamp_micros();
            }
            _ => {
                tracing::warn!("start failed, status: {}", self.status);
            }
            
        }
    }

    pub(crate) fn done(&mut self) {
        match self.status {
            Status::Doing => {
                self.status = Status::Done;
                self.end_time = Local::now().timestamp_micros();
            }
            _ => {
                tracing::warn!("done failed, status: {}", self.status);
            }
        }
    }
}

pub struct TracingInfoManager {
    pub(crate) tracing_infos: AHashMap<&'static str, TracingInfo>,
}

impl Display for TracingInfoManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (key, tracing_info) in self.tracing_infos.iter() {
            write!(f, "key: {}, tracing_info {}; ", key, tracing_info)?;
        }

        Ok(())
    }
    
}

impl TracingInfoManager {
    pub(crate) fn new() -> Self {
        Self {
            tracing_infos: AHashMap::new(),
        }
    }

    pub(crate) fn add_tracing_info(&mut self, key: &'static str) {
        self.tracing_infos.insert(key, TracingInfo::new());
    }

    pub(crate) fn start(&mut self, key: &'static str) {
        if let Some(tracing_info) = self.tracing_infos.get_mut(key) {
            tracing_info.start();
        } else {
            tracing::warn!("start failed, key: {} not found", key);
        }
    }

    pub(crate) fn done(&mut self, key: &'static str) {
        if let Some(tracing_info) = self.tracing_infos.get_mut(key) {
            tracing_info.done();
        } else {
            tracing::warn!("done failed, key: {} not found", key);
        }
    }

    pub(crate) fn get_tracing_info(&self, key: &'static str) -> anyhow::Result<&TracingInfo> {
        self.tracing_infos
            .get(key)
            .ok_or_else(|| anyhow::anyhow!("not found {} in tracing_infos", key))
    }
}