use crate::bot::{
    Message, MessageCtx, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam,
};
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
        Vec::new()
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

    async fn run(&mut self, _variation: usize) -> Option<Vec<Message>> {
        None
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(100)]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            triggers: Some(vec!["/help".to_string()]),
            ..ModuleCapabilities::default()
        }
    }

    async fn trigger(&mut self, _message: &str) -> Option<Vec<MessageResponse>> {
        trace!("responding to /help");
        let command_list = self
            .commands
            .iter()
            .map(|command| format!("- {}\n", command))
            .fold(String::new(), |mut acc, command| {
                acc.push_str(command.as_str());
                acc
            });
        let response = format!("Available commands are:\n{}", command_list);
        Some(vec![response])
    }

    async fn send_message(&mut self, _messages: &[Message]) {}

    async fn read_message(&mut self) -> Option<Vec<MessageCtx>> {
        None
    }

    async fn resp_message(&mut self, _parent: MessageCtx, _message: Message) {}
}

#[derive(Clone)]
pub struct Help {
    commands: HashSet<String>,
}

impl Help {
    pub fn new() -> Result<Self, VarError> {
        Ok(Help {
            commands: HashSet::new(),
        })
    }
}
