use reqwest::Client;
use tokio::time::Duration;

const DEFAULT_TIMEOUT_MS: u64 = 10_000;

pub fn request_agent() -> Result<Client, reqwest::Error> {
    let default_duration = Duration::from_millis(DEFAULT_TIMEOUT_MS);
    Client::builder().timeout(default_duration).build()
}
