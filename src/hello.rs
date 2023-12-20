use crate::bot::{Message, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam};
use crate::webex;
use crate::webex::WebexAgent;
use async_trait::async_trait;
use rand::prelude::IteratorRandom;
use std::env::VarError;
use tokio::time::Duration;

#[async_trait]
impl Module for Hello {
    fn name(&self) -> &'static str {
        "hello"
    }

    fn params(&self) -> Vec<ModuleParam> {
        webex::params()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) -> Option<Vec<Message>> {
        if !self.has_skipped_first_time {
            self.has_skipped_first_time = true;
            return None;
        }
        const RMS_QUOTES: &[&str] = &include!("hello_quotes_rms.rs");
        const OTHER_QUOTES: &[(&str, &str)] = &include!("hello_quotes.rs");
        let all_quotes = OTHER_QUOTES
            .iter()
            .copied()
            .chain(RMS_QUOTES.iter().map(|q| ("RMS", *q)));
        let quote = {
            let mut rng = rand::thread_rng();
            match all_quotes.choose(&mut rng) {
                Some((author, quote)) => format!("{} — {}", quote, author),
                None => return None,
            }
        };
        self.webex.say(quote).await;
        None
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities::default()
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        let seven_day_s = 7 * 24 * 60 * 60;
        vec![Duration::from_secs(seven_day_s)]
    }

    async fn trigger(&mut self, _message: &str) -> Option<Vec<MessageResponse>> {
        None
    }

    async fn send_message(&mut self, _messages: &[Message]) {}
}

#[derive(Clone)]
pub struct Hello {
    webex: WebexAgent,
    has_skipped_first_time: bool,
}

impl Hello {
    pub fn new() -> Result<Self, VarError> {
        Ok(Hello {
            webex: WebexAgent::new()?,
            has_skipped_first_time: false,
        })
    }
}
