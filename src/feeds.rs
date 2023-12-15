use crate::bot::{Module, ModuleData, ModuleParam};
use crate::utils::request_agent;
use crate::webex;
use crate::webex::WebexAgent;
use async_trait::async_trait;
use feed_rs::model;
use feed_rs::parser::parse;
use log::{error, info};
use std::cmp::Ordering;
use std::env::{self, VarError};
use std::error::Error;
use tokio::time::Duration;

#[derive(Clone)]
pub struct Feeds {
    feeds: Vec<Feed>,
    webex: WebexAgent,
}

#[async_trait]
impl Module for Feeds {
    fn name(&self) -> &'static str {
        "feeds"
    }

    fn params(&self) -> Vec<ModuleParam> {
        [
            webex::params(),
            vec![
                ModuleParam::new("FEED_0_NAME", "Feed name, can be multiple (0..)", false),
                ModuleParam::new("FEED_0_URL", "Feed URL, can be multiple (0..)", false),
            ],
        ]
        .concat()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn has_needed_params(&self) -> bool {
        true
    }

    async fn run(&mut self, _variation: usize) {
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
            return;
        } else {
            info!("we have {} new feed entries", messages.len());
        }
        for msg in messages {
            self.webex.say(msg).await;
        }
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(3600)]
    }

    async fn trigger(&mut self, _message: &str, _id: &str) {}
}

impl Feeds {
    pub fn new() -> Result<Feeds, VarError> {
        let mut feeds = Feeds {
            feeds: Vec::new(),
            webex: WebexAgent::new()?,
        };
        for i in 0..100 {
            let name = env::var(&format!("FEED_{}_NAME", i));
            let url = env::var(&format!("FEED_{}_URL", i));
            match (name, url) {
                (Ok(name), Ok(url)) => {
                    info!("feed configured: {} ({}), ", name, url);
                    feeds.feeds.push(Feed::new(name, url));
                }
                _ => break,
            }
        }
        Ok(feeds)
    }
}

#[derive(Clone)]
struct Feed {
    pub name: String,
    pub url: String,
    pub latest: Option<model::Entry>,
}

impl Feed {
    pub fn new(name: String, url: String) -> Self {
        Feed {
            name,
            url,
            latest: None,
        }
    }

    pub async fn update(&mut self) -> bool {
        let new_entry = self.last_entry().await;
        let changed = match (&self.latest, &new_entry) {
            (None, None) => false,
            (Some(old), Some(new)) => old.id != new.id,
            (None, Some(_)) => false,
            (Some(_), None) => false,
        };
        if changed {
            self.latest = new_entry;
        }
        changed
    }

    async fn download(&self) -> Result<model::Feed, Box<dyn Error + Send + Sync>> {
        info!("downloading feeds for {}", self.name);
        let body = match request_agent()?.get(&self.url).send().await {
            Ok(body) => body.text().await?,
            Err(err) => {
                error!("cannot read feed located on {}: {}", self.url, err);
                return Err(Box::new(err));
            }
        };
        match parse(body.as_bytes()) {
            Ok(feed) => Ok(feed),
            Err(error) => Err(Box::new(error)),
        }
    }

    async fn last_entry(&self) -> Option<model::Entry> {
        let mut feed = self.download().await.ok()?;
        feed.entries.sort_by(|a, b| {
            if let Some(date_a) = a.published {
                if let Some(date_b) = b.published {
                    return date_b.cmp(&date_a);
                }
            }
            Ordering::Equal
        });
        let entry = feed.entries.first()?;
        Some(entry.clone())
    }

    fn announce(&self) -> Option<String> {
        let entry = self.latest.clone()?;
        let title = entry.title.map(|title| title.content);
        let url = entry.links.first().map(|link| link.href.clone());
        Some(match (title, url) {
            (None, None) => format!("New post on {}", self.name),
            (None, Some(url)) => format!("New post on [{}]({})", self.name, url),
            (Some(title), None) => format!("New post on {}: {}", self.name, title),
            (Some(title), Some(url)) => format!("{}: [{}]({})", self.name, title, url),
        })
    }
}
