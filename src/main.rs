/* Copyright Outscale SAS */

mod bot;
mod feed;
mod github;
mod osc;
mod roll;
mod webex;
mod ollama;

use bot::Bot;
use log::error;
use std::process::exit;

#[tokio::main]
pub async fn main() {
    env_logger::init();
    let bot = match Bot::load() {
        Some(b) => b,
        None => {
            error!("bot requirements are not met. exiting.");
            exit(1);
        }
    };
    if let Err(e) = bot.check().await {
        error!("error: {}", e);
        exit(1);
    }
    bot.run().await;
}
