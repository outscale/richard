use crate::utils::request_agent;
use crate::webex::WebexAgent;
use log::error;
use std::env::VarError;
use tokio::time::sleep;
use tokio::time::Duration;

use lazy_static::lazy_static;
use std::process::exit;
use std::sync::Arc;
use tokio::sync::RwLock;

static API_DOC_URL: &str = "https://docs.outscale.com/en/userguide/Home.html";
static OMI_DOC_URL: &str = "https://docs.outscale.com/en/userguide/Official-OMIs-Reference.html";

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
    static ref MODULE: Arc<RwLock<Webpages>> = init();
}

fn init() -> Arc<RwLock<Webpages>> {
    match Webpages::new() {
        Ok(h) => Arc::new(RwLock::new(h)),
        Err(err) => {
            error!("cannot initialize module, missing var {:#}", err);
            exit(1);
        }
    }
}
#[derive(Clone)]
struct Webpages {
    pages: Vec<Webpage>,
    webex: WebexAgent,
}

impl Webpages {
    fn new() -> Result<Self, VarError> {
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

    async fn run(&mut self) {
        loop {
            self.check_pages().await;
            sleep(Duration::from_secs(600)).await;
        }
    }

    async fn run_trigger(&mut self, _message: &str, _parent_message: &str) {}

    async fn check_pages(&mut self) {
        for page in self.pages.iter_mut() {
            if page.changed().await {
                let message = format!("{} has changed ({})", page.name, page.url);
                self.webex.say(message).await;
            }
        }
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
