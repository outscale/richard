use crate::bot::{Module, ModuleCapabilities, ModuleData, ModuleParam};
use crate::webex;
use crate::webex::WebexAgent;
use async_trait::async_trait;
use log::trace;
use std::collections::HashSet;
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

    async fn module_offering(&mut self, modules: &[ModuleData]) {
        for module in modules {
            if let Some(triggers) = module.capabilities.triggers.as_ref() {
                for trigger in triggers {
                    self.commands.insert(trigger.clone());
                }
            }
        }
    }

    async fn run(&mut self, _variation: usize) {}

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(100)]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            triggers: Some(vec!["/help".to_string()]),
        }
    }

    async fn trigger(&mut self, _message: &str, id: &str) {
        trace!("responding to /help");
        let command_list = self
            .commands
            .iter()
            .map(|command| format!("- {}\n", command))
            .fold(String::new(), |mut acc, command| {
                acc.push_str(command.as_str());
                acc
            });
        self.webex
            .respond(
                format!("Available commands are:\n{}", command_list).as_str(),
                id,
            )
            .await;
    }
}

#[derive(Clone)]
pub struct Help {
    webex: WebexAgent,
    commands: HashSet<String>,
}

impl Help {
    pub fn new() -> Result<Self, VarError> {
        Ok(Help {
            webex: WebexAgent::new()?,
            commands: HashSet::new(),
        })
    }
}
