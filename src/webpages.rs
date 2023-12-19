use crate::bot::{Module, ModuleData, ModuleParam};
use crate::utils::request_agent;
use crate::webex;
use crate::webex::WebexAgent;
use async_trait::async_trait;
use log::{error, info};
use std::env;
use std::env::VarError;
use tokio::time::Duration;

#[async_trait]
impl Module for Webpages {
    fn name(&self) -> &'static str {
        "webpages"
    }

    fn params(&self) -> Vec<ModuleParam> {
        [
            webex::params(),
            vec![
                ModuleParam::new(
                    "WEBPAGES_0_NAME",
                    "Webpage name, can be multiple (0..)",
                    false,
                ),
                ModuleParam::new(
                    "WEBPAGES_0_URL",
                    "Webpage URL, can be multiple (0..)",
                    false,
                ),
            ],
        ]
        .concat()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) {
        for page in self.pages.iter_mut() {
            if page.changed().await {
                let message = format!("[{}]({}) has changed", page.name, page.url);
                self.webex.say_markdown(message).await;
            }
        }
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(60)]
    }

    async fn trigger(&mut self, _message: &str, _id: &str) {}
}

#[derive(Clone)]
pub struct Webpages {
    pages: Vec<Webpage>,
    webex: WebexAgent,
}

impl Webpages {
    pub fn new() -> Result<Self, VarError> {
        let mut webpages = Webpages {
            pages: Vec::new(),
            webex: WebexAgent::new()?,
        };
        for i in 0..100 {
            let name = env::var(&format!("WEBPAGES_{}_NAME", i));
            let url = env::var(&format!("WEBPAGES_{}_URL", i));
            match (name, url) {
                (Ok(name), Ok(url)) => {
                    info!("webpage configured: '{}' on url '{}'", name, url);
                    let new_webpage = Webpage::new(name.as_str(), url.as_str());
                    webpages.pages.push(new_webpage);
                }
                _ => break,
            }
        }
        Ok(webpages)
    }
}

#[derive(Clone)]
struct Webpage {
    name: String,
    url: String,
    content: Option<String>,
}

impl Webpage {
    fn new(name: &str, url: &str) -> Webpage {
        Webpage {
            name: name.into(),
            url: url.into(),
            content: None,
        }
    }

    async fn changed(&mut self) -> bool {
        let agent = match request_agent() {
            Ok(agent) => agent,
            Err(err) => {
                error!("{:#?}", err);
                return false;
            }
        };
        let result = match agent.get(self.url.clone()).send().await {
            Ok(res) => res,
            Err(err) => {
                error!("{:#?}", err);
                return false;
            }
        };
        let body = match result.text().await {
            Ok(res) => res,
            Err(err) => {
                error!("{:#?}", err);
                return false;
            }
        };

        let mut changed = false;
        if let Some(content) = &self.content {
            if content.len() != body.len() || *content != body {
                changed = true;
            }
        }
        self.content = Some(body);
        changed
    }
}
