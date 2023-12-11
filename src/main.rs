/* Copyright Outscale SAS */

mod bot;
mod feeds;
mod github;
mod hello;
mod ollama;
mod osc;
mod roll;
mod utils;
mod webex;
mod webpages;

use bot::Bot;
use log::error;
use std::process::exit;

#[tokio::main]
pub async fn main() {
    env_logger::init();
    let bot = match Bot::load() {
        Ok(b) => b,
        Err(err) => {
            error!("bot requirements are not met. Missing var {}", err);
            exit(1);
        }
    };
    if let Err(e) = bot.check().await {
        error!("error: {}", e);
        exit(1);
    }
    bot.run().await;
}
