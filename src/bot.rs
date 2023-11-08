use crate::github::Github;
use crate::webex;
use crate::osc;
use crate::roll;
use crate::feed::Feed;
use crate::ollama::Ollama;
use log::{debug, error, info, warn};
use std::env;
use tokio::time::sleep;
use std::time::Duration;
use reqwest::Client;
use std::error::Error;
use tokio::task::JoinSet;

const HIGH_ERROR_RATE: f32 = 0.1;

static API_DOC_URL: &str = "https://docs.outscale.com/en/userguide/Home.html";
static OMI_DOC_URL: &str = "https://docs.outscale.com/en/userguide/Official-OMIs-Reference.html";
const DEFAULT_TIMEOUT_MS: u64 = 10_000;

pub fn request_agent() -> Result<Client, reqwest::Error> {
    let default_duration = Duration::from_millis(DEFAULT_TIMEOUT_MS);
    Client::builder().timeout(default_duration).build()
}

#[derive(Clone)]
pub struct Bot {
    webex_agent: webex::WebexAgent,
    endpoints: Vec<osc::Endpoint>,
    api_page: Option<String>,
    omi_page: Option<String>,
    github: Github,
    feeds: Vec<Feed>,
}

impl Bot {
    pub fn load() -> Option<Self> {
        let webex_token = load_env("WEBEX_TOKEN")?;
        let webex_room_id = load_env("WEBEX_ROOM_ID")?;
        let github_token = load_env("GITHUB_TOKEN")?;
        Some(Bot {
            webex_agent: webex::WebexAgent::new(webex_token, webex_room_id),
            endpoints: Bot::load_endpoints(),
            api_page: None,
            omi_page: None,
            github: Github::new(github_token),
            feeds: Bot::load_feeds(),
        })
    }

    pub fn load_endpoints() -> Vec<osc::Endpoint> {
        let mut endpoints = Vec::new();
        for i in 0..100 {
            let name = load_env(&format!("REGION_{}_NAME", i));
            let endpoint = load_env(&format!("REGION_{}_ENDPOINT", i));
            match (name, endpoint) {
                (Some(name), Some(endpoint)) => {
                    info!("endpoint {} configured", name);
                    let new = osc::Endpoint::new(name, endpoint);
                    endpoints.push(new);
                }
                _ => break,
            }
        }
        endpoints
    }

    pub fn load_feeds() -> Vec<Feed> {
        let mut feeds = Vec::new();
        for i in 0..100 {
            let name = load_env(&format!("FEED_{}_NAME", i));
            let url = load_env(&format!("FEED_{}_URL", i));
            match (name, url) {
                (Some(name), Some(url)) => {
                    info!("feed configured: {} ({}), ", name, url);
                    feeds.push(Feed::new(name, url));
                }
                _ => break,
            }
        }
        feeds
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

    pub async fn hello(&self) {
        const RMS_QUOTES: &[&str] = &include!("rms_quotes.rs");
        let index = rand::random::<usize>() % RMS_QUOTES.len();
        if let Some(quote) = RMS_QUOTES.get(index) {
            self.say(quote.to_string(), false).await;
        }
    }

    pub async fn actions(&mut self) {
        match self.webex_agent.unread_messages().await {
            Ok(messages) => {
                for m in messages.items {
                    info!("received message: {}", m.text);
                    if m.text.contains("help") {
                        self.respond(m.id, "available commands are: ping, status, roll, help").await;
                    } else if m.text.contains("ping") {
                        self.respond(m.id, "pong").await;
                    } else if m.text.contains("status") {
                        self.respond_status(&m.id).await;
                    } else if m.text.contains("roll") {
                        self.action_roll(&m).await;
                    } else if m.text.contains("describe") {
                        self.github.describe_release(m, self.clone()).await
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

    pub async fn check_api_page_update(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let agent = request_agent()?;
        let result = agent.get(API_DOC_URL).send().await?;
        let body = result.text().await?;
        if let Some(api_page) = &self.api_page {
            if api_page.len() != body.len() || *api_page != body {
                self.say(
                    format!("Documentation front page has changed ({})", API_DOC_URL),
                    false,
                ).await;
            }
        }
        self.api_page = Some(body);
        Ok(())
    }

    pub async fn check_omi_page_update(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let agent = request_agent()?;
        let result = agent.get(OMI_DOC_URL).send().await?;
        let body = result.text().await?;
        if let Some(page) = &self.omi_page {
            if page.len() != body.len() || *page != body {
                self.say(
                    format!("OMI page page has changed ({})", OMI_DOC_URL),
                    false,
                ).await;
            }
        }
        self.omi_page = Some(body);
        Ok(())
    }

    pub async fn check_feeds(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut messages: Vec<String> = Vec::new();
        for feed in &mut self.feeds {
            if feed.update().await {
                if let Some(announce) = feed.announce() {
                    messages.push(announce);
                }
            }
        }
        if messages.is_empty() {
            info!("no new feed entry");
            return Ok(())
        } else {
            info!("we have {} new feed entries", messages.len());
        }
        for msg in messages {
            self.say(msg, true).await;
        }
        Ok(())
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
                let day_s = 24 * 60 * 60;
                loop {
                    sleep(Duration::from_secs(7 * day_s)).await;
                    bot.hello().await;
                }
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
                loop {
                    bot.actions().await;
                    sleep(Duration::from_secs(600)).await;
                }
            }));

            let mut bot = self.clone();
            tasks.spawn(tokio::spawn(async move {
                loop {
                    if let Err(err) = bot.check_api_page_update().await {
                        error!("while checking api page update: {}", err);
                    };
                    sleep(Duration::from_secs(600)).await;
                }
            }));

            let mut bot = self.clone();
            tasks.spawn(tokio::spawn(async move {
                loop {
                    if let Err(err) = bot.check_omi_page_update().await {
                        error!("while checking omi page update: {}", err);
                    };
                    sleep(Duration::from_secs(600)).await;
                }
            }));

            let mut bot = self.clone();
            tasks.spawn(tokio::spawn(async move {
                loop {
                    if let Err(err) = bot.check_feeds().await {
                        error!("while checking feeds: {}", err);
                    };
                    sleep(Duration::from_secs(3600)).await;
                }
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

pub fn load_env(env_name: &str) -> Option<String> {
    let value = match env::var(env_name) {
        Ok(v) => v,
        Err(e) => {
            debug!("{}: {}", env_name, e);
            return None;
        }
    };
    if value.is_empty() {
        debug!("{} seems empty", env_name);
        return None;
    }
    debug!("{} is set", env_name);
    Some(value)
}