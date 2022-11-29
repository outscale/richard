/* Copyright Outscale SAS */
use crate::github::{calculate_hash, ReleaseHash};
use clokwerk::Interval::Monday;
use clokwerk::{Scheduler, TimeUnits};
use github::Github;
use log::{error, info, warn, debug};
use rand::seq::IteratorRandom;
use rand::Rng;
use serde::Deserialize;
use std::cmp::min;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::process::exit;
use std::sync::{Arc, RwLock};
use std::thread::sleep;
use std::time::Duration;

mod feed;
mod github;
use feed::Feed;

const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const HIGH_ERROR_RATE: f32 = 0.1;

static API_DOC_URL: &str = "https://docs.outscale.com/en/userguide/Home.html";
static OMI_DOC_URL: &str = "https://docs.outscale.com/en/userguide/Official-OMIs-Reference.html";
static GITHUB_ORG_NAMES: [&str; 2] = ["outscale", "outscale-dev"];

fn request_agent() -> ureq::Agent {
    let default_duration = Duration::from_millis(DEFAULT_TIMEOUT_MS);
    ureq::AgentBuilder::new().timeout(default_duration).build()
}

#[derive(Clone)]
struct WebexAgent {
    auth_header: String,
    room_id: String,
    last_unread_message_date: Option<String>,
}

impl WebexAgent {
    fn new(token: String, room_id: String) -> WebexAgent {
        WebexAgent {
            auth_header: format!("Bearer {}", token),
            room_id,
            last_unread_message_date: None,
        }
    }

    fn post<T: Into<String>>(&self, url: T) -> ureq::Request {
        let url = url.into();
        let agent = request_agent();
        agent.post(&url).set("Authorization", &self.auth_header)
    }

    fn get<T: Into<String>>(&self, url: T) -> ureq::Request {
        let url = url.into();
        let agent = request_agent();
        agent.get(&url).set("Authorization", &self.auth_header)
    }

    fn check(&self) -> Result<(), Box<ureq::Error>> {
        let url = format!(
            "https://webexapis.com/v1/rooms/{}/meetingInfo",
            self.room_id
        );
        if let Err(e) = self.get(&url).call() {
            info!("checking Webex API: KO");
            return Err(Box::new(e));
        }
        info!("checking Webex API: OK");
        Ok(())
    }

    fn say<S: Into<String>>(&self, message: S) -> Result<(), Box<ureq::Error>> {
        self.post("https://webexapis.com/v1/messages")
            .send_json(ureq::json!({
            "roomId": &self.room_id,
            "text": &message.into()
            }))?;
        Ok(())
    }

    fn say_markdown<S: Into<String>>(&self, message: S) -> Result<(), Box<ureq::Error>> {
        self.post("https://webexapis.com/v1/messages")
            .send_json(ureq::json!({
            "roomId": &self.room_id,
            "markdown": &message.into()
            }))?;
        Ok(())
    }

    fn respond<P, M>(&self, parent: P, message: M) -> Result<(), Box<ureq::Error>>
    where
        P: Into<String>,
        M: Into<String>,
    {
        self.post("https://webexapis.com/v1/messages")
            .send_json(ureq::json!({
            "roomId": &self.room_id,
            "parentId": &parent.into(),
            "text": &message.into()
            }))?;
        Ok(())
    }

    fn unread_messages(&mut self) -> Result<WebexMessages, Box<dyn Error>> {
        let url = format!(
            "https://webexapis.com/v1/messages?roomId={}&mentionedPeople=me",
            self.room_id
        );
        let mut res: WebexMessages = self.get(&url).call()?.into_json()?;

        // Sort messages by date
        res.items.sort_by(|a, b| a.created.cmp(&b.created));

        // Filter seen messages
        if let Some(last) = &self.last_unread_message_date {
            res.items.retain(|m| m.created > *last);
        }

        // Update last seen date
        if let Some(m) = res.items.iter().last() {
            let date = Some(m.created.clone());
            if self.last_unread_message_date.is_none() {
                res.items.clear();
            }
            self.last_unread_message_date = date;
        } else if self.last_unread_message_date.is_none() {
            self.last_unread_message_date = Some(String::from("0"));
        }

        Ok(res)
    }
}

#[derive(Clone, Debug, Deserialize)]
struct WebexMessages {
    items: Vec<WebexMessage>,
}

#[derive(Clone, Debug, Deserialize)]
struct WebexMessage {
    id: String,
    text: String,
    created: String,
}

#[derive(Clone)]
struct OscEndpoint {
    name: String,
    endpoint: String,
    version: Option<String>,
    alive: bool,
    access_failure_cnt: u8,
    last_error: Option<OscEndpointError>,
    error_rate_acc: f32,
    error_rate_cnt: u32,
    error_rate: f32,
}

#[derive(Clone, Debug)]
enum OscEndpointError {
    Code(u16),
    Transport(String),
}

impl OscEndpointError {
    fn from_ureq(ureq_error: ureq::Error) -> OscEndpointError {
        match ureq_error {
            ureq::Error::Status(code, _response) => OscEndpointError::Code(code),
            ureq::Error::Transport(transport) => OscEndpointError::Transport(transport.to_string()),
        }
    }
}

impl OscEndpoint {
    // return new version if updated
    // return None on first update
    fn update_version(&mut self) -> Option<String> {
        let version = Some(self.get_version().ok()?);
        let mut ret = None;
        if self.version.is_some() && version != self.version {
            ret = version.clone();
        }
        self.version = version;
        ret
    }

    fn get_version(&self) -> Result<String, Box<dyn Error>> {
        let json: serde_json::Value = request_agent().post(&self.endpoint).call()?.into_json()?;
        Ok(json["Version"].to_string())
    }

    fn update_alive(&mut self) -> (bool, bool) {
        // Schmitt Trigger based on the number of errors
        // https://en.wikipedia.org/wiki/Schmitt_trigger
        const LOW: u8 = 3;
        const HIGH: u8 = 6;
        const MAX_HIGH: u8 = 10;
        let alive_old = self.alive;
        self.access_failure_cnt = match request_agent().post(&self.endpoint).call() {
            Ok(_) => self.access_failure_cnt.saturating_sub(1),
            Err(error) => {
                self.last_error = Some(OscEndpointError::from_ureq(error));
                min(self.access_failure_cnt.saturating_add(1), MAX_HIGH)
            }
        };
        self.alive = match (self.alive, self.access_failure_cnt) {
            (true, HIGH) => false,
            (false, LOW) => true,
            _ => self.alive,
        };
        (alive_old, self.alive)
    }

    fn update_error_rate(&mut self) -> Option<f32> {
        // A simple sliding mean, only providing value once sliding window is full.
        const SIZE: f32 = 100.0;
        self.error_rate_acc = match self.get_version() {
            Ok(_) => self.error_rate_acc + 0.0 - self.error_rate,
            Err(_) => self.error_rate_acc + 1.0 - self.error_rate,
        };
        self.error_rate = self.error_rate_acc / SIZE;
        self.error_rate_cnt = self.error_rate_cnt.saturating_add(1);
        if self.error_rate_cnt >= SIZE as u32 {
            Some(self.error_rate)
        } else {
            None
        }
    }
}

#[derive(Clone)]
struct Bot {
    webex_agent: WebexAgent,
    endpoints: Vec<OscEndpoint>,
    api_page: Option<String>,
    omi_page: Option<String>,
    github: github::Github,
    feeds: Vec<Feed>,
}

impl Bot {
    fn load() -> Option<Self> {
        let webex_token = load_env("WEBEX_TOKEN")?;
        let webex_room_id = load_env("WEBEX_ROOM_ID")?;
        let github_token = load_env("GITHUB_TOKEN")?;
        Some(Bot {
            webex_agent: WebexAgent::new(webex_token, webex_room_id),
            endpoints: Bot::load_endpoints(),
            api_page: None,
            omi_page: None,
            github: Github {
                token: github_token,
                releases: HashMap::new(),
            },
            feeds: Bot::load_feeds(),
        })
    }

    fn load_endpoints() -> Vec<OscEndpoint> {
        let mut endpoints = Vec::new();
        for i in 0..100 {
            let name = load_env(&format!("REGION_{}_NAME", i));
            let endpoint = load_env(&format!("REGION_{}_ENDPOINT", i));
            match (name, endpoint) {
                (Some(name), Some(endpoint)) => {
                    info!("endpoint {} configured", name);
                    let new = OscEndpoint {
                        name,
                        endpoint,
                        version: None,
                        alive: true,
                        access_failure_cnt: 0,
                        last_error: None,
                        error_rate_acc: 0.0,
                        error_rate_cnt: 0,
                        error_rate: 0.0,
                    };
                    endpoints.push(new);
                }
                _ => break,
            }
        }
        endpoints
    }

    fn load_feeds() -> Vec<Feed> {
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

    fn check(&self) -> Result<(), Box<ureq::Error>> {
        self.webex_agent.check()?;
        Ok(())
    }

    fn say<S: Into<String>>(&self, message: S, markdown: bool) {
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

    fn respond<P, M>(&self, parent: P, message: M)
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

    fn say_messages(&self, messages: Vec<String>) {
        for message in messages.iter() {
            self.say(message, false);
        }
    }

    fn endpoint_version_update(&mut self) {
        let mut messages = Vec::<String>::new();
        for endpoint in self.endpoints.iter_mut() {
            info!("updating {}", endpoint.name);
            if let Some(v) = endpoint.update_version() {
                messages.push(format!("New API version on {}: {}", endpoint.name, v));
            }
        }
        self.say_messages(messages);
    }

    fn endpoint_error_rate_update(&mut self) {
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

    fn api_online_check(&mut self) {
        let mut messages = Vec::<String>::new();
        for endpoint in self.endpoints.iter_mut() {
            match endpoint.update_alive() {
                (true, false) => match &endpoint.last_error {
		    Some(error) => match error {
			OscEndpointError::Code(503) => messages.push(format!("API on {} has been very properly put in maintenance mode by the wonderful ops team, thanks for your understanding", endpoint.name)),
			OscEndpointError::Code(other) => messages.push(format!("API on {} region is down (error code: {})", endpoint.name, other)),
			OscEndpointError::Transport(transport) => messages.push(format!("API on {} region seems down (transport error: {})", endpoint.name, transport)),
		    },
		    None => messages.push(format!("API on {} region seems down (no reason found)", endpoint.name)),
		},
                (false, true) => messages.push(format!("API on {} region is up", endpoint.name)),
                _ => {}
            };
            info!(
                "checking if {} region is alive: {}",
                endpoint.name, endpoint.alive
            );
        }
        self.say_messages(messages);
    }

    fn hello(&self) {
        const RMS_QUOTES: &[&str] = &include!("rms_quotes.rs");
        let mut rng = rand::thread_rng();
        if let Some(quote) = RMS_QUOTES.iter().choose(&mut rng) {
            self.say(&quote.to_string(), false);
        }
    }

    fn actions(&mut self) {
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
                    } else {
                        info!("ignoring message");
                    }
                }
            }
            Err(e) => error!("reading messages {}", e),
        };
    }

    fn respond_failure(&mut self, message: &WebexMessage) {
        self.respond(message.id.clone(), "I can't do that Dave.");
    }

    fn action_roll_help(&mut self, message: &WebexMessage) {
        self.respond(
            message.id.clone(),
            "roll <dices> : roll one or more dices where '<dice>' is formated like 1d20.",
        );
    }

    fn action_roll(&mut self, message: &WebexMessage) {
        let first_item_after_roll = match message.text.split("roll").nth(1) {
            Some(roll) => roll,
            None => {
                self.action_roll_help(message);
                return;
            }
        };
        let dices = match first_item_after_roll.split(' ').nth(1) {
            Some(dices) => dices,
            None => {
                self.action_roll_help(message);
                return;
            }
        };
        info!("dices: {}", dices);

        let mut iter = dices.split('d');
        let count_str = match iter.next() {
            Some(count) => count,
            None => {
                self.action_roll_help(message);
                return;
            }
        };
        let faces_str = match iter.next() {
            Some(faces) => faces,
            None => {
                self.action_roll_help(message);
                return;
            }
        };
        let count = match count_str.parse::<usize>() {
            Ok(c) => c,
            Err(_) => {
                self.action_roll_help(message);
                return;
            }
        };
        let faces = match faces_str.parse::<usize>() {
            Ok(f) => f,
            Err(_) => {
                self.action_roll_help(message);
                return;
            }
        };

        if count == 0 || count > 1_000 || faces == 0 || faces > 1000 {
            self.respond_failure(message);
            return;
        }

        let mut rng = rand::thread_rng();
        let mut total = 0;
        let mut output = format!("roll {}d{}: ", count, faces);
        if count > 1 && count < 100 {
            output.push('(');
        }
        for _ in 0..count {
            let roll = rng.gen_range(1..faces + 1);
            if count > 1 && count < 100 {
                output.push_str(format!("{}+", roll).as_str());
            }
            total += roll;
        }
        if count > 1 && count < 100 {
            output.pop();
            output.push_str(") = ");
        }
        output.push_str(format!("{}", total).as_str());
        self.respond(message.id.clone(), &output);
    }

    fn respond_status<S: Into<String>>(&self, parent: S) {
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

    fn check_api_page_update(&mut self) {
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

    fn check_omi_page_update(&mut self) {
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

    fn check_github_release(&mut self) {
        for org_name in GITHUB_ORG_NAMES {
            info!("retrieving all repos from {}", org_name);

            let repos = match self.github.get_all_repos(org_name) {
                Some(value) => value,
                None => continue,
            };
            for repo in repos {
                if repo.is_not_maintained() {
                    continue;
                }
                info!("retrieving latest release for {}/{}", org_name, repo.name);
                let name = &repo.full_name;
                match self.github.get_releases(name) {
                    None => {
                        // Error while retrieving the release
                        if self.github.releases.get(name).is_some() {
                            continue;
                        }
                        self.github.releases.insert(name.to_string(), None);
                    }
                    Some(releases) => match self.github.releases.get(name) {
                        None => {
                            let mut release_hashs: Vec<ReleaseHash> = Vec::new();
                            for release in releases {
                                if release.is_not_official() {
                                    continue;
                                }
                                let release_hash = calculate_hash(&release);
                                release_hashs.push(release_hash)
                            }
                            self.github
                                .releases
                                .insert(name.to_string(), Some(release_hashs));
                        }
                        Some(None) => {
                            let mut release_hashs: Vec<ReleaseHash> = Vec::new();
                            for release in releases {
                                if release.is_not_official() {
                                    continue;
                                }
                                let release_hash = calculate_hash(&release);
                                release_hashs.push(release_hash);
                            }
                            self.github
                                .releases
                                .insert(name.to_string(), Some(release_hashs));
                        }
                        Some(Some(previous_releases)) => {
                            let mut release_hashs: Vec<ReleaseHash> = Vec::new();
                            for release in releases {
                                if release.is_not_official() {
                                    continue;
                                }
                                let release_hash = calculate_hash(&release);
                                release_hashs.push(release_hash);
                                if previous_releases.contains(&release_hash) {
                                    continue;
                                }
                                info!("got release for {} with tag {}", name, release.tag_name);
                                self.say(release.get_notification_message(&repo), true);
                            }
                            self.github
                                .releases
                                .insert(name.to_string(), Some(release_hashs));
                        }
                    },
                }
            }
        }
    }

    fn check_feeds(&mut self) {
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

    let sb = shared_bot;
    scheduler.every(600.seconds()).run(move || {
        if let Ok(mut bot) = sb.write() {
            bot.check_github_release();
        }
    });

    loop {
        scheduler.run_pending();
        sleep(Duration::from_millis(100));
    }
}

fn load_env(env_name: &str) -> Option<String> {
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

fn main() {
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
