use crate::webex::WebexAgent;
use lazy_static::lazy_static;
use log::error;
use log::trace;
use rand::Rng;
use std::env::VarError;
use std::process::exit;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tokio::time::Duration;

pub async fn run() {
    MODULE.write().await.run().await;
}

pub async fn run_trigger(message: &str, parent_message: &str) {
    MODULE
        .write()
        .await
        .run_trigger(message, parent_message)
        .await
}

lazy_static! {
    static ref MODULE: Arc<RwLock<Roll>> = init();
}

fn init() -> Arc<RwLock<Roll>> {
    match Roll::new() {
        Ok(h) => Arc::new(RwLock::new(h)),
        Err(err) => {
            error!("cannot initialize module, missing var {:#}", err);
            exit(1);
        }
    }
}

#[derive(Clone)]
pub struct Roll {
    webex: WebexAgent,
}

impl Roll {
    fn new() -> Result<Self, VarError> {
        Ok(Roll {
            webex: WebexAgent::new()?,
        })
    }

    async fn run(&self) {
        loop {
            sleep(Duration::from_secs(1000)).await;
        }
    }

    async fn run_trigger(&mut self, message: &str, parent_message: &str) {
        if !message.contains("roll") {
            return;
        }
        let response = Roll::gen(&message.into()).unwrap_or(Roll::help().into());
        self.webex.respond(&response, parent_message).await;
    }

    fn gen(request: &String) -> Option<String> {
        trace!("asking to roll {}", request);
        let first_item_after_roll = request.split("roll").nth(1)?;
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
