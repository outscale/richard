/* Copyright Outscale SAS */

use clokwerk::Interval::Monday;
use clokwerk::{Scheduler, TimeUnits};
use github::Github;
use log::{debug, error, info, warn};
use rand::seq::IteratorRandom;

use std::env;
use std::process::exit;
use std::sync::{Arc, RwLock};
use std::thread::sleep;
use std::time::Duration;
mod feed;
mod github;
mod osc;
mod roll;
mod webex;
use feed::Feed;

const HIGH_ERROR_RATE: f32 = 0.1;

static API_DOC_URL: &str = "https://docs.outscale.com/en/userguide/Home.html";
static OMI_DOC_URL: &str = "https://docs.outscale.com/en/userguide/Official-OMIs-Reference.html";
const DEFAULT_TIMEOUT_MS: u64 = 10_000;

pub fn request_agent() -> ureq::Agent {
    let default_duration = Duration::from_millis(DEFAULT_TIMEOUT_MS);
    ureq::AgentBuilder::new().timeout(default_duration).build()
}

#[derive(Clone)]
pub struct Bot {
    webex_agent: webex::WebexAgent,
    endpoints: Vec<osc::Endpoint>,
    api_page: Option<String>,
    omi_page: Option<String>,
    github: github::Github,
    feeds: Vec<Feed>,
}

impl Bot {
    pub fn load() -> Option<Self> {
        let webex_token = load_env("WEBEX_TOKEN")?;
        let webex_room_id = load_env("WEBEX_ROOM_ID")?;
        let github_token = load_env("GITHUB_TOKEN")?;
        Some(Bot {
            webex_agent: webex::WebexAgent::new(webex_token, webex_room_id),
            endpoints: Bot::load_endpoints(),
            api_page: None,
            omi_page: None,
            github: Github::new(github_token),
            feeds: Bot::load_feeds(),
        })
    }

    pub fn load_endpoints() -> Vec<osc::Endpoint> {
        let mut endpoints = Vec::new();
        for i in 0..100 {
            let name = load_env(&format!("REGION_{}_NAME", i));
            let endpoint = load_env(&format!("REGION_{}_ENDPOINT", i));
            match (name, endpoint) {
                (Some(name), Some(endpoint)) => {
                    info!("endpoint {} configured", name);
                    let new = osc::Endpoint::new(name, endpoint);
                    endpoints.push(new);
                }
                _ => break,
            }
        }
        endpoints
    }

    pub fn load_feeds() -> Vec<Feed> {
        let mut feeds = Vec::new();
        for i in 0..100 {
            let name = load_env(&format!("FEED_{}_NAME", i));
            let url = load_env(&format!("FEED_{}_URL", i));
            match (name, url) {
                (Some(name), Some(url)) => {
                    info!("feed configured: {} ({}), ", name, url);
                    feeds.push(Feed::new(name, url));
                }
                _ => break,
            }
        }
        feeds
    }

    pub fn check(&self) -> Result<(), Box<ureq::Error>> {
        self.webex_agent.check()?;
        Ok(())
    }

    pub fn say<S: Into<String>>(&self, message: S, markdown: bool) {
        let message = message.into();
        info!("bot says: {}", message);
        if markdown {
            if let Err(e) = self.webex_agent.say_markdown(message) {
                error!("{}", e);
            }
        } else if let Err(e) = self.webex_agent.say(message) {
            error!("{}", e);
        }
    }

    pub fn respond<P, M>(&self, parent: P, message: M)
    where
        P: Into<String>,
        M: Into<String>,
    {
        let parent = parent.into();
        let message = message.into();
        info!("bot respond: {}", message);
        if let Err(e) = self.webex_agent.respond(parent, message) {
            error!("{}", e);
        }
    }

    pub fn say_messages(&self, messages: Vec<String>) {
        for message in messages.iter() {
            self.say(message, false);
        }
    }

    pub fn endpoint_version_update(&mut self) {
        let mut messages = Vec::<String>::new();
        for endpoint in self.endpoints.iter_mut() {
            info!("updating {}", endpoint.name);
            if let Some(v) = endpoint.update_version() {
                messages.push(format!("New API version on {}: {}", endpoint.name, v));
            }
        }
        self.say_messages(messages);
    }

    pub fn endpoint_error_rate_update(&mut self) {
        for endpoint in self.endpoints.iter_mut() {
            if let Some(error_rate) = endpoint.update_error_rate() {
                if error_rate > HIGH_ERROR_RATE {
                    warn!(
                        "high error rate on {}: {:?}%",
                        endpoint.name,
                        (error_rate * 100.0) as u32
                    );
                }
            }
        }
    }

    pub fn api_online_check(&mut self) {
        let mut messages = Vec::<String>::new();
        for endpoint in self.endpoints.iter_mut() {
            if let Some(response) = endpoint.alive() {
                messages.push(response);
            }
        }
        self.say_messages(messages);
    }

    pub fn hello(&self) {
        const RMS_QUOTES: &[&str] = &include!("rms_quotes.rs");
        let mut rng = rand::thread_rng();
        if let Some(quote) = RMS_QUOTES.iter().choose(&mut rng) {
            self.say(&quote.to_string(), false);
        }
    }

    pub fn actions(&mut self) {
        match self.webex_agent.unread_messages() {
            Ok(messages) => {
                for m in messages.items {
                    info!("received message: {}", m.text);
                    if m.text.contains("help") {
                        // Do not mention emacs
                        self.respond(m.id, "available commands are: ping, status, roll, help");
                    } else if m.text.contains("ping") {
                        self.respond(m.id, "pong");
                    } else if m.text.contains("status") {
                        self.respond_status(&m.id);
                    } else if m.text.contains("emacs") {
                        self.respond(m.id, "You should consider repentance. See https://www.gnu.org/fun/jokes/gospel.html")
                    } else if m.text.contains("roll") {
                        self.action_roll(&m);
                    } else if m.text.contains("describe") {
                        self.github.describe_release(m, self.clone())
                    } else {
                        info!("ignoring message");
                    }
                }
            }
            Err(e) => error!("reading messages {}", e),
        };
    }

    fn action_roll(&mut self, message: &webex::WebexMessage) {
        let Some(response) = roll::gen(&message.text) else {
            self.respond(message.id.clone(), roll::help());
            return;
        };
        self.respond(message.id.clone(), &response);
    }

    pub fn respond_status<S: Into<String>>(&self, parent: S) {
        let mut response = String::new();
        for e in &self.endpoints {
            let version = match &e.version {
                Some(v) => v.clone(),

                None => "unkown".to_string(),
            };
            let s = format!(
                "{}: alive={}, version={}, error_rate={}\n",
                e.name, e.alive, version, e.error_rate
            );
            response.push_str(s.as_str());
        }
        self.respond(parent, response);
    }

    pub fn check_api_page_update(&mut self) {
        let agent = request_agent();
        let req = match agent.get(API_DOC_URL).call() {
            Ok(req) => req,
            Err(e) => {
                error!(
                    "cannot download documentation URL containing API release notes: {}",
                    e
                );
                return;
            }
        };
        let body = match req.into_string() {
            Err(e) => {
                error!(
                    "error: cannot download documentation URL containing API release notes: {}",
                    e
                );
                return;
            }
            Ok(body) => body,
        };
        if let Some(api_page) = &self.api_page {
            if api_page.len() != body.len() || *api_page != body {
                self.say(
                    format!("Documentation front page has changed ({})", API_DOC_URL),
                    false,
                );
            }
        }
        self.api_page = Some(body);
    }

    pub fn check_omi_page_update(&mut self) {
        let agent = request_agent();
        let req = match agent.get(OMI_DOC_URL).call() {
            Ok(req) => req,
            Err(e) => {
                error!(
                    "error: cannot download documentation URL containing OMI details: {}",
                    e
                );
                return;
            }
        };
        let body = match req.into_string() {
            Err(e) => {
                error!(
                    "error: cannot download documentation URL containing OMI details: {}",
                    e
                );
                return;
            }
            Ok(body) => body,
        };
        if let Some(page) = &self.omi_page {
            if page.len() != body.len() || *page != body {
                self.say(
                    format!("OMI page page has changed ({})", OMI_DOC_URL),
                    false,
                );
            }
        }
        self.omi_page = Some(body);
    }

    pub fn check_feeds(&mut self) {
        let mut messages: Vec<String> = Vec::new();
        for feed in &mut self.feeds {
            if feed.update() {
                if let Some(announce) = feed.announce() {
                    messages.push(announce);
                }
            }
        }
        if messages.is_empty() {
            info!("no new feed entry");
            return;
        } else {
            info!("we have {} new feed entries", messages.len());
        }
        for msg in messages {
            self.say(msg, true);
        }
    }
}

fn run_scheduler(bot: Bot) {
    let mut scheduler = Scheduler::new();
    let webex_agent = bot.webex_agent.clone();
    let shared_bot = Arc::new(RwLock::new(bot));

    let sb = shared_bot.clone();
    scheduler.every(600.seconds()).run(move || {
        if let Ok(mut bot) = sb.write() {
            bot.endpoint_version_update();
        }
    });

    let sb = shared_bot.clone();
    scheduler.every(2.seconds()).run(move || {
        if let Ok(mut bot) = sb.write() {
            bot.endpoint_error_rate_update();
        }
    });

    let sb = shared_bot.clone();
    scheduler.every(Monday).at("08:00 am").run(move || {
        if let Ok(bot) = sb.read() {
            bot.hello();
        }
    });

    let sb = shared_bot.clone();
    scheduler.every(20.second()).run(move || {
        if let Ok(mut bot) = sb.write() {
            bot.api_online_check();
        }
    });

    let sb = shared_bot.clone();
    scheduler.every(10.second()).run(move || {
        if let Ok(mut bot) = sb.write() {
            bot.actions();
        }
    });

    let sb = shared_bot.clone();
    scheduler.every(600.seconds()).run(move || {
        if let Ok(mut bot) = sb.write() {
            bot.check_api_page_update();
        }
    });

    let sb = shared_bot.clone();
    scheduler.every(600.seconds()).run(move || {
        if let Ok(mut bot) = sb.write() {
            bot.check_omi_page_update();
        }
    });

    let sb = shared_bot.clone();
    scheduler.every(3600.seconds()).run(move || {
        if let Ok(mut bot) = sb.write() {
            bot.check_feeds();
        }
    });

    let sb = shared_bot.clone();
    scheduler.every(600.seconds()).run(move || {
        if let Ok(mut bot) = sb.write() {
            bot.github
                .check_specific_github_release(webex_agent.clone());
            bot.github.check_github_release(webex_agent.clone());
        }
    });

    loop {
        scheduler.run_pending();
        sleep(Duration::from_millis(100));
    }
}

pub fn load_env(env_name: &str) -> Option<String> {
    let value = match env::var(env_name) {
        Ok(v) => v,
        Err(e) => {
            debug!("{}: {}", env_name, e);
            return None;
        }
    };
    if value.is_empty() {
        debug!("{} seems empty", env_name);
        return None;
    }
    debug!("{} is set", env_name);
    Some(value)
}

pub fn main() {
    env_logger::init();
    let bot = match Bot::load() {
        Some(b) => b,
        None => {
            error!("bot requirements are not met. exiting.");
            exit(1);
        }
    };
    if let Err(e) = bot.check() {
        error!("error: {}", e);
        exit(1);
    }

    run_scheduler(bot);
}
