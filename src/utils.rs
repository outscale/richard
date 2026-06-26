use std::{sync::LazyLock, time::Duration};

use reqwest::Client;

static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);
static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .user_agent(USER_AGENT)
        .tcp_keepalive(Duration::from_secs(30))
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(90))
        .http2_adaptive_window(true)
        .build()
        .expect("Could not build HTTP client")
});

pub fn request_agent() -> Result<Client, reqwest::Error> {
    let client = &*CLIENT;
    Ok(client.clone())
}
