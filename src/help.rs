use crate::bot::{Module, ModuleData, ModuleParam};
use crate::webex;
use crate::webex::WebexAgent;
use async_trait::async_trait;
use log::trace;
use std::env::VarError;
use tokio::time::Duration;

#[async_trait]
impl Module for Help {
    fn name(&self) -> &'static str {
        "help"
    }

    fn params(&self) -> Vec<ModuleParam> {
        webex::params()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn has_needed_params(&self) -> bool {
        true
    }

    async fn run(&mut self, _variation: usize) {}

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(100)]
    }

    async fn trigger(&mut self, message: &str, id: &str) {
        if !message.contains("/help") {
            trace!("ignoring message {}", message);
            return;
        }
        trace!("responding to help");
        self.webex
            .respond("available commands are: ping, status, roll, help", id)
            .await;
    }
}

#[derive(Clone)]
pub struct Help {
    webex: WebexAgent,
}

impl Help {
    pub fn new() -> Result<Self, VarError> {
        Ok(Help {
            webex: WebexAgent::new()?,
        })
    }
}
