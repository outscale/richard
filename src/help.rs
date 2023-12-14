use crate::webex::WebexAgent;
use log::trace;
use std::env::VarError;

use crate::bot::{Module, ModuleParam, SharedModule};
use async_trait::async_trait;
use tokio::time::Duration;

#[async_trait]
impl Module for Help {
    fn name(&self) -> &'static str {
        "ping"
    }

    fn params(&self) -> Vec<ModuleParam> {
        vec![]
    }

    async fn module_offering(&mut self, _modules: &Vec<SharedModule>) {}

    async fn has_needed_params(&self) -> bool {
        true
    }

    async fn run(&mut self, _variation: usize) {}

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(100)]
    }

    async fn trigger(&mut self, message: &str, id: &str) {
        if !message.contains("help") {
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
