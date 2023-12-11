use crate::webex::WebexAgent;
use std::env::VarError;

#[derive(Clone)]
pub struct Ping {
    webex: WebexAgent,
}

impl Ping {
    pub fn new() -> Result<Self, VarError> {
        Ok(Ping {
            webex: WebexAgent::new()?,
        })
    }

    pub async fn run_trigger(&mut self, message: &str, parent_message: &str) {
        if !message.contains("ping") {
            return;
        }
        self.webex.respond(parent_message, "pong").await;
    }
}
