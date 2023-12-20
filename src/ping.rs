use crate::bot::{MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam};
use async_trait::async_trait;
use log::trace;
use std::env::VarError;
use tokio::time::Duration;

#[async_trait]
impl Module for Ping {
    fn name(&self) -> &'static str {
        "ping"
    }

    fn params(&self) -> Vec<ModuleParam> {
        Vec::new()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) {}

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(100)]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            triggers: Some(vec!["/ping".to_string()]),
            catch_non_triggered: false,
            catch_all: false,
        }
    }

    async fn trigger(&mut self, _message: &str) -> Option<Vec<MessageResponse>> {
        trace!("responding to /ping");
        Some(vec!["pong".to_string()])
    }
}

#[derive(Clone)]
pub struct Ping {}

impl Ping {
    pub fn new() -> Result<Self, VarError> {
        Ok(Ping {})
    }
}
