use crate::bot::{
    Message, MessageCtx, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam,
};
use crate::utils::request_agent;
use async_trait::async_trait;
use log::{error, info, warn};
use std::env;
use std::env::VarError;
use tokio::sync::Mutex;
use tokio::time::Duration;

#[async_trait]
impl Module for Webpages {
    fn name(&self) -> &'static str {
        "webpages"
    }

    fn params(&self) -> Vec<ModuleParam> {
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
        ]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities::default()
    }

    async fn module_offering(&self, _modules: &[ModuleData]) {}

    async fn run(&self, _variation: usize) -> Option<Vec<Message>> {
        let mut messages = Vec::new();
        for page in self.pages.iter() {
            if page.changed().await {
                let message = format!("[{}]({}) has changed", page.name, page.url);
                messages.push(message);
            }
        }
        if messages.is_empty() {
            return None;
        }
        Some(messages)
    }

    fn variation_durations(&self) -> Vec<Duration> {
        vec![Duration::from_secs(60)]
    }

    async fn trigger(&self, _message: &str) -> Option<Vec<MessageResponse>> {
        None
    }

    async fn send_message(&self, _messages: &[Message]) {}

    async fn read_message(&self) -> Option<Vec<MessageCtx>> {
        None
    }

    async fn resp_message(&self, _parent: MessageCtx, _message: Message) {}
}

#[derive(Default)]
pub struct Webpages {
    pages: Vec<Webpage>,
}

impl Webpages {
    pub fn new() -> Result<Self, VarError> {
        let mut webpages = Webpages::default();
        for i in 0..100 {
            let name = env::var(format!("WEBPAGES_{}_NAME", i));
            let url = env::var(format!("WEBPAGES_{}_URL", i));
            match (name, url) {
                (Ok(name), Ok(url)) => {
                    info!("webpage configured: '{}' on url '{}'", name, url);
                    let new_webpage = Webpage::new(name.as_str(), url.as_str());
                    webpages.pages.push(new_webpage);
                }
                _ => break,
            }
        }
        if webpages.pages.is_empty() {
            warn!("webpages module enabled bot not configuration provided");
        }
        Ok(webpages)
    }
}

struct Webpage {
    name: String,
    url: String,
    content: Mutex<Option<String>>,
}

impl Webpage {
    fn new(name: &str, url: &str) -> Webpage {
        Webpage {
            name: name.into(),
            url: url.into(),
            content: Mutex::new(None),
        }
    }

    async fn changed(&self) -> bool {
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

        let mut lock = self.content.lock().await;
        let mut changed = false;
        if let Some(ref content) = *lock {
            if content.len() != body.len() || *content != body {
                changed = true;
            }
        }
        *lock = Some(body);
        changed
    }
}
