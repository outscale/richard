/* Copyright Outscale SAS */

use bot::Bot;
use clap::{command, Arg, ArgAction};
use log::info;
use std::process::exit;

mod bot;
mod down_detectors;
mod feeds;
mod github_orgs;
mod github_repos;
mod hello;
mod help;
mod ollama;
mod outscale_api_versions;
mod ping;
mod roll;
mod triggers;
mod utils;
mod webex;
mod webpages;

#[tokio::main]
pub async fn main() {
    env_logger::init();
    let mut bot = Bot::new().await;

    let matches = command!()
        .arg(
            Arg::new("show-params")
                .short('p')
                .long("show-params")
                .action(ArgAction::SetTrue),
        )
        .get_matches();

    if matches.get_flag("show-params") {
        eprintln!("{}", bot.help().await);
        exit(0);
    }
    info!("bot will now run");
    bot.run().await;
}
