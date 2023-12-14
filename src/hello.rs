use crate::webex::WebexAgent;
use rand::prelude::IteratorRandom;
use std::env::VarError;
use tokio::time::sleep;
use tokio::time::Duration;

use lazy_static::lazy_static;
use log::error;
use std::process::exit;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn run() {
    loop {
        let day_s = 24 * 60 * 60;
        sleep(Duration::from_secs(7 * day_s)).await;
        {
            MODULE.write().await.run().await;
        }
    }
}

pub async fn run_trigger(message: &str, parent_message: &str) {
    MODULE
        .write()
        .await
        .run_trigger(message, parent_message)
        .await
}

lazy_static! {
    static ref MODULE: Arc<RwLock<Hello>> = init();
}

fn init() -> Arc<RwLock<Hello>> {
    match Hello::new() {
        Ok(h) => Arc::new(RwLock::new(h)),
        Err(err) => {
            error!("cannot initialize module, missing var {:#}", err);
            exit(1);
        }
    }
}

#[derive(Clone)]
pub struct Hello {
    webex: WebexAgent,
}

impl Hello {
    fn new() -> Result<Self, VarError> {
        Ok(Hello {
            webex: WebexAgent::new()?,
        })
    }

    async fn hello(&self) {
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

    async fn run(&self) {
        self.hello().await;
    }

    async fn run_trigger(&mut self, _message: &str, _parent_message: &str) {}
}
