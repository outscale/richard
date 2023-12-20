use crate::bot::{
    Message, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam, MessageCtx,
};
use async_trait::async_trait;
use log::trace;
use std::env::VarError;
use tokio::time::Duration;

#[async_trait]
impl Module for Ping {
    fn name(&self) -> &'static str {
        "ping"
    }

    fn params(&self) -> Vec<ModuleParam> {
        Vec::new()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) -> Option<Vec<Message>> {
        None
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(100)]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            triggers: Some(vec!["/ping".to_string()]),
            ..ModuleCapabilities::default()
        }
    }

    async fn trigger(&mut self, _message: &str) -> Option<Vec<MessageResponse>> {
        trace!("responding to /ping");
        Some(vec!["pong".to_string()])
    }

    async fn send_message(&mut self, _messages: &[Message]) {}

    async fn read_message(&mut self) -> Option<Vec<MessageCtx>> {
        None
    }

    async fn resp_message(&mut self, _parent: MessageCtx, _message: Message) {}
}

#[derive(Clone)]
pub struct Ping {}

impl Ping {
    pub fn new() -> Result<Self, VarError> {
        Ok(Ping {})
    }
}
