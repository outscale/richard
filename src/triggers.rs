use crate::bot::{Module, ModuleCapabilities, ModuleData, ModuleParam};
use crate::webex;
use crate::webex::WebexAgent;
use async_trait::async_trait;
use log::error;
use std::env::VarError;
use tokio::time::Duration;

#[derive(Clone)]
pub struct Triggers {
    webex: WebexAgent,
    all_modules: Vec<ModuleData>,
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
        "triggers"
    }

    fn params(&self) -> Vec<ModuleParam> {
        webex::params()
    }

    async fn module_offering(&mut self, modules: &[ModuleData]) {
        self.all_modules = modules
            .iter()
            .filter(|module| module.name != "triggers")
            .filter(|module| module.capabilities.triggers.is_some())
            .cloned()
            .collect();
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities { triggers: None }
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
                let Some(triggers) = module.capabilities.triggers.as_ref() else {
                    continue;
                };
                let mut module_rw = module.module.write().await;
                if triggers.is_empty() {
                    module_rw.trigger(&message.text, &message.id).await;
                    continue;
                }
                for trigger in triggers.iter() {
                    if message.text.contains(trigger) {
                        module_rw.trigger(&message.text, &message.id).await;
                    }
                }
            }
        }
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(10)]
    }

    async fn trigger(&mut self, _message: &str, _id: &str) {}
}
