use crate::webex::WebexAgent;
use lazy_static::lazy_static;
use log::error;
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
    static ref MODULE: Arc<RwLock<Ping>> = init();
}

fn init() -> Arc<RwLock<Ping>> {
    match Ping::new() {
        Ok(h) => Arc::new(RwLock::new(h)),
        Err(err) => {
            error!("cannot initialize module, missing var {:#}", err);
            exit(1);
        }
    }
}

#[derive(Clone)]
pub struct Ping {
    webex: WebexAgent,
}

impl Ping {
    pub fn new() -> Result<Self, VarError> {
        Ok(Ping {
            webex: WebexAgent::new()?,
        })
    }

    async fn run_trigger(&mut self, message: &str, parent_message: &str) {
        if !message.contains("ping") {
            return;
        }
        self.webex.respond("pong", parent_message).await;
    }

    async fn run(&self) {
        loop {
            sleep(Duration::from_secs(1000)).await;
        }
    }
}
