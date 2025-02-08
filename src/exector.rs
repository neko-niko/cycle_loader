
use std::sync::Arc;

use async_trait::async_trait;

#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute(&self) -> anyhow::Result<()>;
    fn name(&self) -> &'static str;
}

pub fn exector_wapper<T: Executor + 'static>(executor: T) -> Arc<dyn Executor> {
    Arc::new(executor)
}

