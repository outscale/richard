use crate::bot::Message;
use crate::bot::{MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam};
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
}

#[async_trait]
impl Module for Ollama {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn params(&self) -> Vec<ModuleParam> {
        vec![
            ModuleParam::new("OLLAMA_MODEL_NAME", "Ollama model name to use", true),
            ModuleParam::new("OLLAMA_URL", "ollama URL to query", true),
        ]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            catch_non_triggered: true,
            ..ModuleCapabilities::default()
        }
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) -> Option<Vec<Message>> {
        None
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(100)]
    }

    async fn trigger(&mut self, message: &str) -> Option<Vec<MessageResponse>> {
        trace!("respond on any other message");
        let response = match self.query(message).await {
            Ok(resp) => resp,
            Err(err) => {
                error!("ollama responded: {:#?}", err);
                "Sorry, I can't respond to that right now.".to_string()
            }
        };
        Some(vec![response])
    }

    async fn send_message(&mut self, _messages: Vec<String>) {}
}

impl Ollama {
    pub fn new() -> Result<Ollama, VarError> {
        Ok(Ollama {
            model: env::var("OLLAMA_MODEL_NAME")?,
            endpoint: env::var("OLLAMA_URL")?,
            context: Vec::new(),
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
