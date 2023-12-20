use crate::bot::{
    Message, MessageCtx, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam,
};
use async_trait::async_trait;
use log::trace;
use std::env::VarError;
use tokio::time::Duration;

#[derive(Clone, Default)]
pub struct Triggers {
    trigger_modules: Vec<ModuleData>,
    chat_modules: Vec<ModuleData>,
}

#[async_trait]
impl Module for Triggers {
    fn name(&self) -> &'static str {
        "triggers"
    }

    fn params(&self) -> Vec<ModuleParam> {
        Vec::new()
    }

    async fn module_offering(&mut self, modules: &[ModuleData]) {
        self.trigger_modules = modules
            .iter()
            .filter(|module| module.name != "triggers")
            .filter(|module| {
                module.capabilities.triggers.is_some()
                    || module.capabilities.catch_non_triggered
                    || module.capabilities.catch_all
            })
            .cloned()
            .collect();
        self.chat_modules = modules
            .iter()
            .filter(|module| module.name != "triggers")
            .filter(|module| module.capabilities.read_message || module.capabilities.resp_message)
            .cloned()
            .collect();
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities::default()
    }

    async fn run(&mut self, _variation: usize) -> Option<Vec<Message>> {
        let modules = self.chat_modules.clone();
        for chat_module in modules {
            self.grab_messages(&chat_module).await;
        }
        None
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(10)]
    }

    async fn trigger(&mut self, _message: &str) -> Option<Vec<MessageResponse>> {
        None
    }

    async fn send_message(&mut self, _messages: &[Message]) {}

    async fn read_message(&mut self) -> Option<Vec<MessageCtx>> {
        None
    }

    async fn resp_message(&mut self, _parent: MessageCtx, _message: Message) {}
}

impl Triggers {
    pub fn new() -> Result<Self, VarError> {
        Ok(Triggers::default())
    }

    async fn grab_messages(&mut self, chat_module: &ModuleData) {
        let mut module = chat_module.module.write().await;
        let new_messages = match module.read_message().await {
            Some(messages) => messages,
            None => return,
        };
        drop(module);

        for message in new_messages {
            trace!("getting new message '{}'", message.content);
            let mut responses = Vec::<MessageResponse>::new();
            let mut triggered = false;
            for module in self.trigger_modules.iter() {
                if module.capabilities.catch_all {
                    trace!("module {} catch all message", module.name);
                    let mut module_rw = module.module.write().await;
                    if let Some(mut mod_responses) = module_rw.trigger(&message.content).await {
                        responses.append(&mut mod_responses);
                    }
                }
                if let Some(triggers) = module.capabilities.triggers.as_ref() {
                    for trigger in triggers.iter() {
                        if message.content.contains(trigger) {
                            trace!(
                                "module {} is triggered because message contains '{}'",
                                module.name,
                                trigger
                            );
                            triggered = true;
                            let mut module_rw = module.module.write().await;
                            if let Some(mut mod_responses) =
                                module_rw.trigger(&message.content).await
                            {
                                responses.append(&mut mod_responses);
                            }
                        }
                    }
                }
            }
            if !triggered {
                trace!("no module has been triggered by message");
                for module in self.trigger_modules.iter() {
                    if module.capabilities.catch_non_triggered {
                        trace!("module {} will catch non triggered message", module.name);
                        let mut module_rw = module.module.write().await;
                        if let Some(mut mod_responses) = module_rw.trigger(&message.content).await {
                            responses.append(&mut mod_responses);
                        }
                    }
                }
            }
            let mut module = chat_module.module.write().await;
            for response in responses {
                module.resp_message(message.clone(), response).await
            }
        }
    }
}
