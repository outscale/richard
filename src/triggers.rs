use crate::bot::{Module, ModuleParam, SharedModule};
use crate::webex::WebexAgent;
use async_trait::async_trait;
use log::{error, trace};
use std::env::VarError;
use tokio::time::Duration;

#[derive(Clone)]
pub struct Triggers {
    webex: WebexAgent,
    all_modules: Vec<SharedModule>,
}

impl Triggers {
    pub fn new() -> Result<Self, VarError> {
        Ok(Triggers {
            webex: WebexAgent::new()?,
            all_modules: Vec::new(),
        })
    }
}
#[async_trait]
impl Module for Triggers {
    fn name(&self) -> &'static str {
        "ping"
    }

    fn params(&self) -> Vec<ModuleParam> {
        vec![]
    }

    async fn module_offering(&mut self, modules: Vec<SharedModule>) {
        self.all_modules = modules;
    }

    async fn has_needed_params(&self) -> bool {
        true
    }

    async fn run(&mut self, _variation: usize) {
        let new_messages = match self.webex.unread_messages().await {
            Ok(messages) => messages,
            Err(err) => {
                error!("reading messages: {:#?}", err);
                return;
            }
        };
        for message in new_messages.items {
            for module in self.all_modules.iter() {
                let mut module_rw = module.write().await;
                module_rw.trigger(&message.text, &message.id).await;
            }
        }
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(10)]
    }

    async fn trigger(&mut self, _message: &str, _id: &str) {}
}
