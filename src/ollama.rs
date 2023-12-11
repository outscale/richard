use crate::webex::WebexAgent;
use log::error;
use log::trace;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{env::VarError, error::Error, time::Duration};

#[derive(Clone)]
pub struct Ollama {
    model: String,
    endpoint: String,
    context: Vec<usize>,
    webex: WebexAgent,
}

impl Ollama {
    pub fn new() -> Result<Ollama, VarError> {
        Ok(Ollama {
            model: "richard".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            context: Vec::new(),
            webex: WebexAgent::new()?,
        })
    }

    pub async fn query(&mut self, prompt: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
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
        match self.query(message).await {
            Ok(message) => self.webex.respond(parent_message, &message).await,
            Err(err) => error!("ollama responded: {:#?}", err),
        };
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
