use crate::bot::{Module, ModuleParam, SharedModule};
use crate::utils::request_agent;
use crate::webex;
use crate::webex::WebexAgent;
use async_trait::async_trait;
use log::error;
use std::env::VarError;
use tokio::time::Duration;

static API_DOC_URL: &str = "https://docs.outscale.com/en/userguide/Home.html";
static OMI_DOC_URL: &str = "https://docs.outscale.com/en/userguide/Official-OMIs-Reference.html";

#[async_trait]
impl Module for Webpages {
    fn name(&self) -> &'static str {
        "ping"
    }

    fn params(&self) -> Vec<ModuleParam> {
        webex::params()
    }

    async fn module_offering(&mut self, _modules: &[SharedModule]) {}

    async fn has_needed_params(&self) -> bool {
        true
    }

    async fn run(&mut self, _variation: usize) {
        for page in self.pages.iter_mut() {
            if page.changed().await {
                let message = format!("{} has changed ({})", page.name, page.url);
                self.webex.say(message).await;
            }
        }
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(600)]
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
        let webpages = Webpages {
            // TODO: set by env var listing
            pages: vec![
                Webpage::new("Documentation front page", API_DOC_URL),
                Webpage::new("OMI page", OMI_DOC_URL),
            ],
            webex: WebexAgent::new()?,
        };
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
        let result = match agent.get(API_DOC_URL).send().await {
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
