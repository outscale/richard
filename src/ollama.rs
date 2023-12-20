use crate::bot::{Module, ModuleCapabilities, ModuleData, ModuleParam};
use crate::webex;
use crate::webex::WebexAgent;
use async_trait::async_trait;
use log::error;
use log::trace;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::{env::VarError, error::Error, time::Duration};

#[derive(Clone)]
pub struct Ollama {
    model: String,
    endpoint: String,
    context: Vec<usize>,
    webex: WebexAgent,
}

#[async_trait]
impl Module for Ollama {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn params(&self) -> Vec<ModuleParam> {
        [
            webex::params(),
            vec![
                ModuleParam::new("OLLAMA_MODEL_NAME", "Ollama model name to use", true),
                ModuleParam::new("OLLAMA_URL", "ollama URL to query", true),
            ],
        ]
        .concat()
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            triggers: None,
            catch_non_triggered: true,
            catch_all: false,
        }
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) {}

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(100)]
    }

    async fn trigger(&mut self, message: &str, id: &str) {
        trace!("respond on any other message");
        match self.query(message).await {
            Ok(message) => self.webex.respond(&message, id).await,
            Err(err) => {
                error!("ollama responded: {:#?}", err);
                self.webex
                    .respond("Sorry, I can't respond to that right now.", id)
                    .await
            }
        };
    }
}

impl Ollama {
    pub fn new() -> Result<Ollama, VarError> {
        Ok(Ollama {
            model: env::var("OLLAMA_MODEL_NAME")?,
            endpoint: env::var("OLLAMA_URL")?,
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
