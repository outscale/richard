use crate::endpoints;
use crate::feeds;
use crate::github;
use crate::hello;
use crate::help;
use crate::ollama;
use crate::ping;
use crate::roll;
use crate::triggers;
use crate::webex;
use crate::webpages;
use log::error;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio::time::sleep;

pub async fn run() {
    let mut tasks = JoinSet::new();
    tasks.spawn(tokio::spawn(async move {
        help::run().await;
    }));
    tasks.spawn(tokio::spawn(async move {
        roll::run().await;
    }));
    tasks.spawn(tokio::spawn(async move {
        ping::run().await;
    }));
    tasks.spawn(tokio::spawn(async move {
        ollama::run().await;
    }));
    tasks.spawn(tokio::spawn(async move {
        endpoints::run_version().await;
    }));
    tasks.spawn(tokio::spawn(async move {
        endpoints::run_error_rate().await;
    }));
    tasks.spawn(tokio::spawn(async move {
        endpoints::run_alive().await;
    }));
    tasks.spawn(tokio::spawn(async move {
        hello::run().await;
    }));
    tasks.spawn(tokio::spawn(async move {
        webpages::run().await;
    }));
    tasks.spawn(tokio::spawn(async move {
        feeds::run().await;
    }));
    tasks.spawn(tokio::spawn(async move {
        github::run().await;
    }));
    tasks.spawn(tokio::spawn(async move {
        github::run().await;
    }));
    tasks.spawn(tokio::spawn(async move {
        triggers::run().await;
    }));

    loop {
        tasks.join_next().await;
        error!("this should not happen :)");
        sleep(Duration::from_secs(1)).await;
    }
}

pub async fn check() -> bool {
    let Ok(w) = webex::WebexAgent::new() else {
        return false;
    };
    w.check().await
}
