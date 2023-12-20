use crate::bot::{
    Message, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam, MessageCtx,
};
use async_trait::async_trait;
use log::trace;
use rand::Rng;
use std::env::VarError;
use tokio::time::Duration;

#[derive(Clone)]
pub struct Roll {}

#[async_trait]
impl Module for Roll {
    fn name(&self) -> &'static str {
        "roll"
    }

    fn params(&self) -> Vec<ModuleParam> {
        Vec::new()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) -> Option<Vec<Message>> {
        None
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(9999)]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            triggers: Some(vec!["/roll".to_string()]),
            ..ModuleCapabilities::default()
        }
    }

    async fn trigger(&mut self, message: &str) -> Option<Vec<MessageResponse>> {
        let response = Roll::gen(&message.into()).unwrap_or(Roll::help().into());
        Some(vec![response])
    }

    async fn send_message(&mut self, _messages: &[Message]) {}

    async fn read_message(&mut self) -> Option<Vec<MessageCtx>> {
        None
    }

    async fn resp_message(&mut self, _parent: MessageCtx, _message: Message) {}
}

impl Roll {
    pub fn new() -> Result<Self, VarError> {
        Ok(Roll {})
    }

    fn gen(request: &String) -> Option<String> {
        trace!("asking to roll {}", request);
        let first_item_after_roll = request.split("/roll").nth(1)?;
        let dices = first_item_after_roll.split(' ').nth(1)?;
        trace!("dices: {}", dices);

        let mut iter = dices.split('d');
        let count_str = iter.next()?;
        trace!("dice count: {}", count_str);
        let faces_str = iter.next()?;
        trace!("faces: {}", faces_str);
        let count = count_str.parse::<usize>().ok()?;
        let faces = faces_str.parse::<usize>().ok()?;

        if count == 0 || count > 1_000 || faces == 0 || faces > 1000 {
            return None;
        }

        let mut rng = rand::thread_rng();
        let mut total = 0;
        let mut output = format!("roll {}d{}: ", count, faces);
        if count > 1 && count < 100 {
            output.push('(');
        }
        for _ in 0..count {
            let roll = rng.gen_range(1..faces + 1);
            if count > 1 && count < 100 {
                output.push_str(format!("{}+", roll).as_str());
            }
            total += roll;
        }
        if count > 1 && count < 100 {
            output.pop();
            output.push_str(") = ");
        }
        output.push_str(format!("{}", total).as_str());
        trace!("roll result message: {}", output);
        Some(output)
    }

    fn help() -> &'static str {
        "roll <dices> : roll one or more dices where '<dice>' is formated like 1d20."
    }
}
