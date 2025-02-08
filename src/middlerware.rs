use std::{future::Future, pin::Pin};


use crate::exector::Executor;

pub type Middlerware =
    Box<dyn (
        Fn(Box<dyn Executor>) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>
    )>;
