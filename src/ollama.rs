use log::trace;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{error::Error, time::Duration};

pub struct Ollama {
    model: String,
    endpoint: String,
    context: Vec<usize>,
}

impl Default for Ollama {
    fn default() -> Self {
        Ollama {
            model: "richard".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            context: Vec::new(),
        }
    }
}

impl Ollama {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test::block_on;

    #[test]
    fn api_query() {
        let mut ollama = Ollama::default();
        assert!(!block_on(ollama.query("Hello Richard!")).unwrap().is_empty());
    }
}
