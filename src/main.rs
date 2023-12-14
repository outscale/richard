/* Copyright Outscale SAS */

use std::process::exit;
use log::{error, info};

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
    let mut bot = bot::Bot::new();
    if !bot.ready().await {
        error!("some bot modules does not have requiered parameters");
        exit(1);
    }
    info!("bot will now run");
    bot.run().await;
}
