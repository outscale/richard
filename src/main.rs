/* Copyright Outscale SAS */

use std::process::exit;

mod bot;
mod endpoints;
mod feeds;
mod github;
mod hello;
mod help;
mod ollama;
mod ping;
mod roll;
mod triggers;
mod utils;
mod webex;
mod webpages;

#[tokio::main]
pub async fn main() {
    env_logger::init();
    if !bot::check().await {
        exit(1);
    }
    bot::run().await;
}
