use crate::webex::WebexAgent;
use rand::prelude::IteratorRandom;
use std::env::VarError;
use tokio::time::sleep;
use tokio::time::Duration;

#[derive(Clone)]
pub struct Hello {
    webex: WebexAgent,
}

impl Hello {
    pub fn new() -> Result<Self, VarError> {
        Ok(Hello {
            webex: WebexAgent::new()?,
        })
    }

    pub async fn hello(&self) {
        const RMS_QUOTES: &[&str] = &include!("rms_quotes.rs");
        const OTHER_QUOTES: &[(&str, &str)] = &include!("quotes.rs");
        let all_quotes = OTHER_QUOTES
            .iter()
            .copied()
            .chain(RMS_QUOTES.iter().map(|q| ("RMS", *q)));
        let quote = {
            let mut rng = rand::thread_rng();
            match all_quotes.choose(&mut rng) {
                Some((author, quote)) => format!("{} — {}", quote, author),
                None => return,
            }
        };
        self.webex.say(quote).await;
    }

    pub async fn run(&self) {
        let day_s = 24 * 60 * 60;
        loop {
            sleep(Duration::from_secs(7 * day_s)).await;
            self.hello().await;
        }
    }
}
