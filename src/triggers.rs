use crate::bot::{
    Message, MessageCtx, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam,
    SharedModule,
};
use async_trait::async_trait;
use log::trace;
use std::env::VarError;
use tokio::{sync::RwLock, time::Duration};

#[derive(Clone, Default)]
pub struct InnerTriggers {
    trigger_modules: Vec<ModuleData>,
    chat_modules: Vec<ModuleData>,
}

#[derive(Default)]
pub struct Triggers {
    inner: RwLock<InnerTriggers>,
}

#[async_trait]
impl Module for Triggers {
    fn name(&self) -> &'static str {
        "triggers"
    }

    fn params(&self) -> Vec<ModuleParam> {
        Vec::new()
    }

    async fn module_offering(&self, modules: &[ModuleData]) {
        let trigger_modules = modules
            .iter()
            .filter(|module| module.name != "triggers")
            .filter(|module| {
                module.capabilities.triggers.is_some()
                    || module.capabilities.catch_non_triggered
                    || module.capabilities.catch_all
            })
            .cloned()
            .collect();
        let chat_modules = modules
            .iter()
            .filter(|module| module.name != "triggers")
            .filter(|module| module.capabilities.read_message || module.capabilities.resp_message)
            .cloned()
            .collect();

        let inner = InnerTriggers {
            trigger_modules,
            chat_modules,
        };

        let mut lock = self.inner.write().await;
        *lock = inner;
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities::default()
    }

    async fn run(&self, _variation: usize) -> Option<Vec<Message>> {
        let (trigger_modules, chat_modules): (Vec<ModuleData>, Vec<SharedModule>) = {
            let lock = self.inner.read().await;
            (
                lock.trigger_modules.clone(),
                lock.chat_modules.iter().map(|m| m.module.clone()).collect(),
            )
        };
        for chat_module in chat_modules {
            run_chat_module(&trigger_modules, chat_module).await;
        }
        None
    }

    fn variation_durations(&self) -> Vec<Duration> {
        vec![Duration::from_secs(10)]
    }

    async fn trigger(&self, _message: &str) -> Option<Vec<MessageResponse>> {
        None
    }

    async fn send_message(&self, _messages: &[Message]) {}

    async fn read_message(&self) -> Option<Vec<MessageCtx>> {
        None
    }

    async fn resp_message(&self, _parent: MessageCtx, _message: Message) {}
}

impl Triggers {
    pub fn new() -> Result<Self, VarError> {
        Ok(Triggers::default())
    }
}

async fn run_chat_module(trigger_modules: &[ModuleData], chat_module: SharedModule) {
    let new_messages = match chat_module.read_message().await {
        Some(messages) if !messages.is_empty() => messages,
        _ => return,
    };

    for message in new_messages {
        trace!("getting new message '{}'", message.content);
        let mut responses = Vec::<MessageResponse>::new();
        let mut triggered = false;
        for trigger_module in trigger_modules.iter() {
            if trigger_module.capabilities.catch_all {
                trace!("module {} catch all message", trigger_module.name);
                if let Some(mut trigger_responses) =
                    trigger_module.module.trigger(&message.content).await
                {
                    responses.append(&mut trigger_responses);
                }
            } else if let Some(triggers) = trigger_module.capabilities.triggers.as_ref() {
                for trigger in triggers.iter() {
                    if message.content.contains(trigger) {
                        trace!(
                            "module {} is triggered because message contains '{}'",
                            trigger_module.name,
                            trigger
                        );
                        triggered = true;
                        if let Some(mut trigger_responses) =
                            trigger_module.module.trigger(&message.content).await
                        {
                            responses.append(&mut trigger_responses);
                        }
                    }
                }
            }
        }
        if !triggered {
            trace!("no module has been triggered by message");
            for trigger_module in trigger_modules.iter() {
                if trigger_module.capabilities.catch_non_triggered {
                    trace!(
                        "module {} will catch non triggered message",
                        trigger_module.name
                    );
                    if let Some(mut trigger_responses) =
                        trigger_module.module.trigger(&message.content).await
                    {
                        responses.append(&mut trigger_responses);
                    }
                }
            }
        }
        for response in responses {
            chat_module.resp_message(message.clone(), response).await
        }
    }
}
