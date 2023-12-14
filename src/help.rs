use lazy_static::lazy_static;

use crate::webex::WebexAgent;
use log::error;
use std::env::VarError;
use std::process::exit;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tokio::time::Duration;

pub async fn run() {
    loop {
        {
            MODULE.write().await.run().await;
        }
        sleep(Duration::from_secs(1000)).await;
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
    static ref MODULE: Arc<RwLock<Help>> = init();
}

fn init() -> Arc<RwLock<Help>> {
    match Help::new() {
        Ok(h) => Arc::new(RwLock::new(h)),
        Err(err) => {
            error!("cannot initialize module, missing var {:#}", err);
            exit(1);
        }
    }
}

#[derive(Clone)]
pub struct Help {
    webex: WebexAgent,
}

impl Help {
    fn new() -> Result<Self, VarError> {
        Ok(Help {
            webex: WebexAgent::new()?,
        })
    }

    async fn run(&self) {}

    async fn run_trigger(&mut self, message: &str, parent_message: &str) {
        if !message.contains("help") {
            return;
        }
        self.webex
            .respond(
                "available commands are: ping, status, roll, help",
                parent_message
            )
            .await;
    }
}
