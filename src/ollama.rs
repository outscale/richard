use crate::webex::WebexAgent;
use log::error;
use log::trace;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{env::VarError, error::Error, time::Duration};

use lazy_static::lazy_static;
use std::process::exit;
use std::sync::Arc;
use tokio::sync::RwLock;

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
    static ref MODULE: Arc<RwLock<Ollama>> = init();
}

fn init() -> Arc<RwLock<Ollama>> {
    match Ollama::new() {
        Ok(h) => Arc::new(RwLock::new(h)),
        Err(err) => {
            error!("cannot initialize module, missing var {:#}", err);
            exit(1);
        }
    }
}

#[derive(Clone)]
struct Ollama {
    model: String,
    endpoint: String,
    context: Vec<usize>,
    webex: WebexAgent,
}

impl Ollama {
    fn new() -> Result<Ollama, VarError> {
        Ok(Ollama {
            model: "richard".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            context: Vec::new(),
            webex: WebexAgent::new()?,
        })
    }

    async fn query(&mut self, prompt: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        trace!("asking richard: {}", prompt);
        let url = format!("{}/api/generate", self.endpoint);
        let query = OllamaQuery {
            prompt: String::from(prompt),
            model: self.model.clone(),
            stream: false,
            context: self.context.clone(),
        };
        let response_body = Client::new()
            .post(url)
            .json(&query)
            .timeout(Duration::from_secs(600))
            .send()
            .await?
            .text()
            .await?;
        trace!("response from ollama API: {}", response_body);
        let response: OllamaResponse = serde_json::from_str(response_body.as_str())?;
        trace!("ollama context was {:#?}", self.context);
        if let Some(context) = response.context {
            self.context = context;
        }
        trace!("ollama context is now {:#?}", self.context);
        Ok(response.response)
    }

    pub async fn run_trigger(&mut self, message: &str, parent_message: &str) {
        let keywords = vec!["status", "help", "ping", "roll"];
        for keyword in keywords {
            if message.contains(keyword) {
                return;
            }
        }

        match self.query(message).await {
            Ok(message) => self.webex.respond(parent_message, &message).await,
            Err(err) => error!("ollama responded: {:#?}", err),
        };
    }

    pub async fn run(&self) {}
}

#[derive(Clone, Debug, Serialize)]
struct OllamaQuery {
    prompt: String,
    model: String,
    stream: bool,
    context: Vec<usize>,
}

#[derive(Clone, Debug, Deserialize)]
struct OllamaResponse {
    response: String,
    context: Option<Vec<usize>>,
}
