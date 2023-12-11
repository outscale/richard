use crate::feeds::Feeds;
use crate::github::Github;
use crate::hello::Hello;
use crate::ollama::Ollama;
use crate::osc;
use crate::roll;
use crate::webex;
use crate::webpages::Webpages;
use log::{debug, error, info, warn};
use std::env;
use std::env::VarError;
use std::error::Error;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio::time::sleep;

const HIGH_ERROR_RATE: f32 = 0.1;
#[derive(Clone)]
pub struct Bot {
    webex_agent: webex::WebexAgent,
    endpoints: Vec<osc::Endpoint>,
    hello: Hello,
    github: Github,
    feeds: Feeds,
    webpages: Webpages,
}

impl Bot {
    pub fn load() -> Result<Self, VarError> {
        Ok(Bot {
            webex_agent: webex::WebexAgent::new()?,
            endpoints: Bot::load_endpoints(),
            hello: Hello::new()?,
            github: Github::new()?,
            feeds: Feeds::new()?,
            webpages: Webpages::new()?,
        })
    }

    pub fn load_endpoints() -> Vec<osc::Endpoint> {
        let mut endpoints = Vec::new();
        for i in 0..100 {
            let name = env::var(&format!("REGION_{}_NAME", i));
            let endpoint = env::var(&format!("REGION_{}_ENDPOINT", i));
            match (name, endpoint) {
                (Ok(name), Ok(endpoint)) => {
                    info!("endpoint {} configured", name);
                    let new = osc::Endpoint::new(name, endpoint);
                    endpoints.push(new);
                }
                _ => break,
            }
        }
        endpoints
    }

    pub async fn check(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.webex_agent.check().await?;
        Ok(())
    }

    pub async fn say<S: Into<String>>(&self, message: S, markdown: bool) {
        let message = message.into();
        info!("bot says: {}", message);
        match markdown {
            true => self.webex_agent.say_markdown(message).await,
            false => self.webex_agent.say(message).await,
        };
    }

    pub async fn respond<P, M>(&self, parent: P, message: M)
    where
        P: Into<String>,
        M: Into<String>,
    {
        let parent = parent.into();
        let message = message.into();
        info!("bot respond: {}", message);
        if let Err(e) = self.webex_agent.respond(parent, message).await {
            error!("{}", e);
        }
    }

    pub async fn say_messages(&self, messages: Vec<String>) {
        for message in messages.iter() {
            self.say(message, false).await;
        }
    }

    pub async fn endpoint_version_update(&mut self) {
        let mut messages = Vec::<String>::new();
        for endpoint in self.endpoints.iter_mut() {
            info!("updating {}", endpoint.name);
            if let Some(v) = endpoint.update_version().await {
                messages.push(format!("New API version on {}: {}", endpoint.name, v));
            }
        }
        self.say_messages(messages).await;
    }

    pub async fn endpoint_error_rate_update(&mut self) {
        for endpoint in self.endpoints.iter_mut() {
            if let Some(error_rate) = endpoint.update_error_rate().await {
                if error_rate > HIGH_ERROR_RATE {
                    warn!(
                        "high error rate on {}: {:?}%",
                        endpoint.name,
                        (error_rate * 100.0) as u32
                    );
                }
            }
        }
    }

    pub async fn api_online_check(&mut self) {
        let mut messages = Vec::<String>::new();
        for endpoint in self.endpoints.iter_mut() {
            if let Some(response) = endpoint.alive().await {
                messages.push(response);
            }
        }
        self.say_messages(messages).await;
    }

    pub async fn actions(&mut self) {
        match self.webex_agent.unread_messages().await {
            Ok(messages) => {
                for m in messages.items {
                    info!("received message: {}", m.text);
                    if m.text.contains("help") {
                        self.respond(m.id, "available commands are: ping, status, roll, help")
                            .await;
                    } else if m.text.contains("ping") {
                        self.respond(m.id, "pong").await;
                    } else if m.text.contains("status") {
                        self.respond_status(&m.id).await;
                    } else if m.text.contains("roll") {
                        self.action_roll(&m).await;
                    } else {
                        let mut ollama = Ollama::default();
                        match ollama.query(&m.text).await {
                            Ok(message) => self.respond(m.id, message).await,
                            Err(err) => error!("ollama responded: {:#?}", err),
                        };
                    }
                }
            }
            Err(e) => error!("reading messages: {}", e),
        };
    }

    async fn action_roll(&mut self, message: &webex::WebexMessage) {
        let Some(response) = roll::gen(&message.text) else {
            self.respond(message.id.clone(), roll::help()).await;
            return;
        };
        self.respond(message.id.clone(), response).await;
    }

    pub async fn respond_status<S: Into<String>>(&self, parent: S) {
        let mut response = String::new();
        for e in &self.endpoints {
            let version = match &e.version {
                Some(v) => v.clone(),

                None => "unkown".to_string(),
            };
            let s = format!(
                "{}: alive={}, version={}, error_rate={}\n",
                e.name, e.alive, version, e.error_rate
            );
            response.push_str(s.as_str());
        }
        self.respond(parent, response).await;
    }

    pub async fn run(self) {
        let mut tasks = JoinSet::new();
        let mut bot = self.clone();
        tasks.spawn(tokio::spawn(async move {
            loop {
                bot.endpoint_version_update().await;
                sleep(Duration::from_secs(600)).await;
            }
        }));

        let mut bot = self.clone();
        tasks.spawn(tokio::spawn(async move {
            loop {
                bot.endpoint_error_rate_update().await;
                sleep(Duration::from_secs(2)).await;
            }
        }));

        let bot = self.clone();
        tasks.spawn(tokio::spawn(async move {
            bot.hello.run().await;
        }));

        let mut bot = self.clone();
        tasks.spawn(tokio::spawn(async move {
            loop {
                bot.api_online_check().await;
                sleep(Duration::from_secs(2)).await;
            }
        }));

        let mut bot = self.clone();
        tasks.spawn(tokio::spawn(async move {
            loop {
                bot.actions().await;
                sleep(Duration::from_secs(10)).await;
            }
        }));

        let mut bot = self.clone();
        tasks.spawn(tokio::spawn(async move {
            bot.webpages.run().await;
        }));

        let mut bot = self.clone();
        tasks.spawn(tokio::spawn(async move {
            bot.feeds.run().await;
        }));

        let mut bot = self.clone();
        let webex = self.webex_agent.clone();
        tasks.spawn(tokio::spawn(async move {
            loop {
                if let Err(err) = bot.github.check_specific_github_release(&webex).await {
                    error!("while checking specific github release: {}", err);
                }
                sleep(Duration::from_secs(600)).await;
            }
        }));

        let mut bot = self.clone();
        let webex = self.webex_agent.clone();
        tasks.spawn(tokio::spawn(async move {
            loop {
                if let Err(err) = bot.github.check_github_release(&webex).await {
                    error!("while checking github release: {}", err);
                };
                sleep(Duration::from_secs(600)).await;
            }
        }));

        loop {
            tasks.join_next().await;
            debug!("this should not happen :)");
            sleep(Duration::from_secs(1)).await;
        }
    }
}
