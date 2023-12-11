use crate::endpoints;
use crate::feeds;
use crate::github;
use crate::hello;
use crate::help;
use crate::ollama;
use crate::ping;
use crate::roll;
use crate::webex::WebexAgent;
use crate::webpages;
use lazy_static::lazy_static;
use log::error;
use log::info;
use std::env::VarError;
use std::process::exit;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tokio::time::Duration;

pub async fn run() {
    MODULE.write().await.run().await;
}

lazy_static! {
    static ref MODULE: Arc<RwLock<Triggers>> = init();
}

fn init() -> Arc<RwLock<Triggers>> {
    match Triggers::new() {
        Ok(h) => Arc::new(RwLock::new(h)),
        Err(err) => {
            error!("cannot initialize module, missing var {:#}", err);
            exit(1);
        }
    }
}

#[derive(Clone)]
struct Triggers {
    webex: WebexAgent,
}

impl Triggers {
    fn new() -> Result<Self, VarError> {
        Ok(Triggers {
            webex: WebexAgent::new()?,
        })
    }

    async fn run(&mut self) {
        loop {
            self.triggers().await;
            sleep(Duration::from_secs(10)).await;
        }
    }

    async fn triggers(&mut self) {
        let new_messages = match self.webex.unread_messages().await {
            Ok(messages) => messages,
            Err(err) => {
                error!("reading messages: {:#?}", err);
                return;
            }
        };

        for m in new_messages.items {
            info!("received message: {}", m.text);
            endpoints::run_trigger(&m.text, &m.id).await;
            roll::run_trigger(&m.text, &m.id).await;
            ping::run_trigger(&m.text, &m.id).await;
            help::run_trigger(&m.text, &m.id).await;
            ollama::run_trigger(&m.text, &m.id).await;
            hello::run_trigger(&m.text, &m.id).await;
            webpages::run_trigger(&m.text, &m.id).await;
            feeds::run_trigger(&m.text, &m.id).await;
            github::run_trigger(&m.text, &m.id).await;
        }
    }
}
