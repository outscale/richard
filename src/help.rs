use crate::webex::WebexAgent;
use std::env::VarError;

#[derive(Clone)]
pub struct Help {
    webex: WebexAgent,
}

impl Help {
    pub fn new() -> Result<Self, VarError> {
        Ok(Help {
            webex: WebexAgent::new()?,
        })
    }

    pub async fn run_trigger(&mut self, message: &str, parent_message: &str) {
        if !message.contains("help") {
            return;
        }
        self.webex
            .respond(
                parent_message,
                "available commands are: ping, status, roll, help",
            )
            .await;
    }
}
