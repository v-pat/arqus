use crate::flow::config::types::{ResolvedFlowConfig, WorkerConfig};
use crate::flow::errors::FlowError;
use crate::flow::schema::types::TaskContract;

pub type WorkerProfile = WorkerConfig;

pub trait Router {
    fn select<'a>(&'a self, contract: &TaskContract) -> &'a WorkerConfig;
}

#[derive(Debug, Clone)]
pub struct DefaultRouter {
    default_worker: WorkerConfig,
}

impl DefaultRouter {
    pub fn new(default_worker: WorkerConfig) -> Self {
        Self { default_worker }
    }

    pub fn from_config(config: &ResolvedFlowConfig) -> Result<Self, FlowError> {
        let worker = config
            .workers
            .iter()
            .find(|w| w.name == config.routing.default_worker)
            .ok_or_else(|| FlowError::UnknownWorker(config.routing.default_worker.clone()))?;
        Ok(Self::new(worker.clone()))
    }
}

impl Router for DefaultRouter {
    fn select<'a>(&'a self, _contract: &TaskContract) -> &'a WorkerConfig {
        &self.default_worker
    }
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
