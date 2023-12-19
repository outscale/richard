use crate::bot::{Module, ModuleCapabilities, ModuleData, ModuleParam};
use crate::webex;
use crate::webex::WebexAgent;
use async_trait::async_trait;
use log::trace;
use rand::Rng;
use std::env::VarError;
use tokio::time::Duration;

#[derive(Clone)]
pub struct Roll {
    webex: WebexAgent,
}

#[async_trait]
impl Module for Roll {
    fn name(&self) -> &'static str {
        "roll"
    }

    fn params(&self) -> Vec<ModuleParam> {
        webex::params()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) {}

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(9999)]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            triggers: Some(vec!["/roll".to_string()]),
        }
    }

    async fn trigger(&mut self, message: &str, id: &str) {
        let response = Roll::gen(&message.into()).unwrap_or(Roll::help().into());
        self.webex.respond(&response, id).await;
    }
}

impl Roll {
    pub fn new() -> Result<Self, VarError> {
        Ok(Roll {
            webex: WebexAgent::new()?,
        })
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
