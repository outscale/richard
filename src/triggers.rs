use crate::webex::WebexAgent;
use std::env::VarError;
use tokio::time::Duration;
use log::{error, trace};
use crate::bot::{Module, SharedModule, ModuleParam};
use async_trait::async_trait;

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

    fn module_offering(&mut self, modules: Vec<SharedModule>) {
        self.all_modules = modules;
    }

    async fn has_needed_params(&self) -> bool {
        true
    }

    async fn run(&mut self) {
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

    async fn cooldown_duration(&mut self) -> Duration {
        Duration::from_secs(10)
    }

    async fn trigger(&mut self, _message: &str, _id: &str) {}
}
