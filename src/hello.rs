use crate::bot::{Module, ModuleParam, SharedModule};
use crate::webex;
use crate::webex::WebexAgent;
use async_trait::async_trait;
use log::trace;
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

    async fn module_offering(&mut self, _modules: &[SharedModule]) {}

    async fn has_needed_params(&self) -> bool {
        true
    }

    async fn run(&mut self, _variation: usize) {
        if !self.has_skipped_first_time {
            self.has_skipped_first_time = true;
            return;
        }
        const RMS_QUOTES: &[&str] = &include!("rms_quotes.rs");
        const OTHER_QUOTES: &[(&str, &str)] = &include!("quotes.rs");
        let all_quotes = OTHER_QUOTES
            .iter()
            .copied()
            .chain(RMS_QUOTES.iter().map(|q| ("RMS", *q)));
        let quote = {
            let mut rng = rand::thread_rng();
            match all_quotes.choose(&mut rng) {
                Some((author, quote)) => format!("{} — {}", quote, author),
                None => return,
            }
        };
        self.webex.say(quote).await;
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        let seven_day_s = 7 * 24 * 60 * 60;
        vec![Duration::from_secs(seven_day_s)]
    }

    async fn trigger(&mut self, message: &str, id: &str) {
        if !message.contains("ping") {
            trace!("ignoring message {}", message);
            return;
        }
        trace!("responding to ping");
        self.webex.respond("pong", id).await;
    }
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
