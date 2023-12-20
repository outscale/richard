use crate::bot::{
    Message, MessageCtx, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam,
    SharedModule,
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
        let chat_modules = self.chat_modules.clone();
        for chat_module in chat_modules {
            self.run_chat_module(chat_module.module.clone()).await;
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

    async fn run_chat_module(&mut self, chat_module: SharedModule) {
        let mut chat_module_rw = chat_module.write().await;
        let new_messages = match chat_module_rw.read_message().await {
            Some(messages) => messages,
            None => return,
        };
        drop(chat_module_rw);

        for message in new_messages {
            trace!("getting new message '{}'", message.content);
            let mut responses = Vec::<MessageResponse>::new();
            let mut triggered = false;
            for trigger_module in self.trigger_modules.iter() {
                if trigger_module.capabilities.catch_all {
                    trace!("module {} catch all message", trigger_module.name);
                    let mut trigger_module_rw = trigger_module.module.write().await;
                    if let Some(mut trigger_responses) =
                        trigger_module_rw.trigger(&message.content).await
                    {
                        responses.append(&mut trigger_responses);
                    }
                }
                if let Some(triggers) = trigger_module.capabilities.triggers.as_ref() {
                    for trigger in triggers.iter() {
                        if message.content.contains(trigger) {
                            trace!(
                                "module {} is triggered because message contains '{}'",
                                trigger_module.name,
                                trigger
                            );
                            triggered = true;
                            let mut trigger_module_rw = trigger_module.module.write().await;
                            if let Some(mut trigger_responses) =
                                trigger_module_rw.trigger(&message.content).await
                            {
                                responses.append(&mut trigger_responses);
                            }
                        }
                    }
                }
            }
            if !triggered {
                trace!("no module has been triggered by message");
                for trigger_module in self.trigger_modules.iter() {
                    if trigger_module.capabilities.catch_non_triggered {
                        trace!(
                            "module {} will catch non triggered message",
                            trigger_module.name
                        );
                        let mut trigger_module_rw = trigger_module.module.write().await;
                        if let Some(mut trigger_responses) =
                            trigger_module_rw.trigger(&message.content).await
                        {
                            responses.append(&mut trigger_responses);
                        }
                    }
                }
            }
            let mut chat_module_rw = chat_module.write().await;
            for response in responses {
                chat_module_rw.resp_message(message.clone(), response).await
            }
        }
    }
}
