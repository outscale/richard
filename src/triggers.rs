use crate::bot::{Module, ModuleCapabilities, ModuleData, ModuleParam};
use crate::webex;
use crate::webex::WebexAgent;
use async_trait::async_trait;
use log::{error, trace};
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
            .filter(|module| {
                module.capabilities.triggers.is_some()
                    || module.capabilities.catch_non_triggered
                    || module.capabilities.catch_all
            })
            .cloned()
            .collect();
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            triggers: None,
            catch_non_triggered: false,
            catch_all: false,
        }
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
            trace!("getting new message '{}'", message.text);
            let mut triggered = false;
            for module in self.all_modules.iter() {
                if module.capabilities.catch_all {
                    trace!("module {} catch all message", module.name);
                    let mut module_rw = module.module.write().await;
                    module_rw.trigger(&message.text, &message.id).await;
                }
                if let Some(triggers) = module.capabilities.triggers.as_ref() {
                    for trigger in triggers.iter() {
                        if message.text.contains(trigger) {
                            trace!(
                                "module {} is triggered because message contains '{}'",
                                module.name,
                                trigger
                            );
                            triggered = true;
                            let mut module_rw = module.module.write().await;
                            module_rw.trigger(&message.text, &message.id).await;
                        }
                    }
                }
            }
            if !triggered {
                trace!("no module has been triggered by message");
                for module in self.all_modules.iter() {
                    if module.capabilities.catch_non_triggered {
                        trace!("module {} will catch non triggered message", module.name);
                        let mut module_rw = module.module.write().await;
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
