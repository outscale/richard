use crate::endpoints;
use crate::feeds::Feeds;
use crate::github::Github;
use crate::hello;
use crate::help;
use crate::ollama;
use crate::ping;
use crate::roll;
use crate::webex::WebexAgent;
use crate::webpages;
use log::{debug, error, info};
use std::env::VarError;
use std::error::Error;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio::time::sleep;

#[derive(Clone)]
pub struct Bot {
    webex: WebexAgent,
    github: Github,
    feeds: Feeds,
}

impl Bot {
    pub fn load() -> Result<Self, VarError> {
        Ok(Bot {
            webex: WebexAgent::new()?,
            github: Github::new()?,
            feeds: Feeds::new()?,
        })
    }

    pub async fn check(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.webex.check().await?;
        Ok(())
    }

    pub async fn actions(&mut self) {
        match self.webex.unread_messages().await {
            Ok(messages) => {
                for m in messages.items {
                    info!("received message: {}", m.text);
                    endpoints::run_trigger(&m.text, &m.id).await;
                    roll::run_trigger(&m.text, &m.id).await;
                    ping::run_trigger(&m.text, &m.id).await;
                    help::run_trigger(&m.text, &m.id).await;
                    ollama::run_trigger(&m.text, &m.id).await;
                    hello::run_trigger(&m.text, &m.id).await;
                    webpages::run_trigger(&m.text, &m.id).await;
                }
            }
            Err(e) => error!("reading messages: {}", e),
        };
    }

    pub async fn run(self) {
        let mut tasks = JoinSet::new();
        tasks.spawn(tokio::spawn(async move {
            help::run().await;
        }));
        tasks.spawn(tokio::spawn(async move {
            roll::run().await;
        }));
        tasks.spawn(tokio::spawn(async move {
            ping::run().await;
        }));
        tasks.spawn(tokio::spawn(async move {
            ollama::run().await;
        }));
        tasks.spawn(tokio::spawn(async move {
            endpoints::run_version().await;
        }));
        tasks.spawn(tokio::spawn(async move {
            endpoints::run_error_rate().await;
        }));
        tasks.spawn(tokio::spawn(async move {
            endpoints::run_alive().await;
        }));
        tasks.spawn(tokio::spawn(async move {
            hello::run().await;
        }));
        tasks.spawn(tokio::spawn(async move {
            webpages::run().await;
        }));

        let mut bot = self.clone();
        tasks.spawn(tokio::spawn(async move {
            bot.feeds.run().await;
        }));

        let mut bot = self.clone();
        tasks.spawn(tokio::spawn(async move {
            bot.github.run().await;
        }));

        let mut bot = self.clone();
        tasks.spawn(tokio::spawn(async move {
            loop {
                bot.actions().await;
                sleep(Duration::from_secs(10)).await;
            }
        }));

        loop {
            tasks.join_next().await;
            debug!("this should not happen :)");
            sleep(Duration::from_secs(1)).await;
        }
    }
}
