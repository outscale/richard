use crate::bot::{Module, ModuleCapabilities, ModuleData, ModuleParam};
use crate::webex;
use crate::webex::WebexAgent;
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
        webex::params()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) {}

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(100)]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            triggers: Some(vec!["/ping".to_string()]),
        }
    }

    async fn trigger(&mut self, message: &str, id: &str) {
        if !message.contains("/ping") {
            trace!("ignoring message {}", message);
            return;
        }
        trace!("responding to ping");
        self.webex.respond("pong", id).await;
    }
}

#[derive(Clone)]
pub struct Ping {
    webex: WebexAgent,
}

impl Ping {
    pub fn new() -> Result<Self, VarError> {
        Ok(Ping {
            webex: WebexAgent::new()?,
        })
    }
}
