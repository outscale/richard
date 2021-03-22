/* Copyright Outscale SAS */
use clokwerk::Interval::Monday;
use clokwerk::{Scheduler, TimeUnits};
use rand::seq::IteratorRandom;
use std::cmp::min;
use std::env;
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;
use ureq;

const DEFAULT_TIMEOUT_MS: u64 = 10_000;

fn request_agent() -> ureq::Agent {
    let default_duration = Duration::from_millis(DEFAULT_TIMEOUT_MS);
    ureq::AgentBuilder::new().timeout(default_duration).build()
}

#[derive(Clone)]
struct OscEndpoint {
    name: String,
    endpoint: String,
    version: Option<String>,
    alive: bool,
    access_failure_cnt: u8,
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
        return ret;
    }

    fn get_version(&self) -> Result<String, ureq::Error> {
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
        self.access_failure_cnt = match self.get_version() {
            Ok(_) => self.access_failure_cnt.saturating_sub(1),
            Err(_) => min(self.access_failure_cnt.saturating_add(1), MAX_HIGH),
        };
        self.alive = match (self.alive, self.access_failure_cnt) {
            (true, HIGH) => false,
            (false, LOW) => true,
            _ => self.alive,
        };
        (alive_old, self.alive)
    }
}

#[derive(Clone)]
struct Bot {
    room_id: String,
    webex_token: String,
    endpoints: Vec<OscEndpoint>,
    debug: bool,
}

impl Bot {
    fn load() -> Option<Self> {
        Some(Bot {
            room_id: Bot::load_env("ROOM_ID", true)?,
            webex_token: Bot::load_env("WEBEX_TOKEN", true)?,
            endpoints: Bot::load_endpoints(),
            debug: Bot::load_debug(),
        })
    }

    fn load_env(env_name: &str, verbose: bool) -> Option<String> {
        let value = match env::var(env_name) {
            Ok(v) => v,
            Err(e) => {
                if verbose {
                    eprintln!("{}: {}", env_name, e);
                }
                return None;
            }
        };
        if value.len() == 0 {
            if verbose {
                eprintln!("{} seems empty", env_name);
            }
            return None;
        }
        if verbose {
            println!("{} is set", env_name);
        }
        return Some(value);
    }

    fn load_debug() -> bool {
        match Bot::load_env("DEBUG", false) {
            Some(_) => {
                println!("DEBUG is set");
                true
            }
            None => false,
        }
    }

    fn load_endpoints() -> Vec<OscEndpoint> {
        let mut endpoints = Vec::new();
        print!("regions configured: ");
        for i in 0..100 {
            let name = Bot::load_env(&format!("REGION_{}_NAME", i), false);
            let endpoint = Bot::load_env(&format!("REGION_{}_ENDPOINT", i), false);
            match (name, endpoint) {
                (Some(name), Some(endpoint)) => {
                    print!("{}, ", name);
                    let new = OscEndpoint {
                        name: name,
                        endpoint: endpoint,
                        version: None,
                        alive: true,
                        access_failure_cnt: 0,
                    };
                    endpoints.push(new);
                }
                _ => break,
            }
        }
        println!("");
        return endpoints;
    }

    fn check(&self) -> Result<(), ureq::Error> {
        print!("checking webex API: ");
        let url = format!(
            "https://webexapis.com/v1/rooms/{}/meetingInfo",
            self.room_id
        );
        let auth = format!("Bearer {}", self.webex_token);
        if let Err(e) = request_agent().get(&url).set("Authorization", &auth).call() {
            println!("KO");
            return Err(e);
        }
        println!("OK");
        return Ok(());
    }

    fn say(&self, message: &String) {
        println!("bot says: {}", message);
        if self.debug {
            return;
        }
        let auth = format!("Bearer {}", self.webex_token);
        let resp = request_agent()
            .post("https://webexapis.com/v1/messages")
            .set("Authorization", &auth)
            .send_json(ureq::json!({
            "roomId": &self.room_id,
            "text": &message
            }));
        if let Err(e) = resp {
            eprintln!("error: {}", e);
        }
    }
    fn say_messages(&self, messages: Vec<String>) {
        for message in messages.iter() {
            self.say(message);
        }
    }

    fn endpoint_version_update(&mut self) {
        let mut messages = Vec::<String>::new();
        for endpoint in self.endpoints.iter_mut() {
            println!("updating {}", endpoint.name);
            if let Some(v) = endpoint.update_version() {
                messages.push(format!("New API version on {}: {}", endpoint.name, v));
            }
        }
        self.say_messages(messages);
    }

    fn api_online_check(&mut self) {
        let mut messages = Vec::<String>::new();
        for endpoint in self.endpoints.iter_mut() {
            print!("checking if {} region is alive: ", endpoint.name);
            match endpoint.update_alive() {
                (true, false) => {
                    messages.push(format!("API on {} region went down", endpoint.name))
                }
                (false, true) => messages.push(format!("API on {} region went up", endpoint.name)),
                _ => {}
            };
            println!("{}", endpoint.alive);
        }
        self.say_messages(messages);
    }

    fn hello(&self) {
        const RMS_QUOTES: &'static [&'static str] = &include!("rms_quotes.rs");
        let mut rng = rand::thread_rng();
        if let Some(quote) = RMS_QUOTES.iter().choose(&mut rng) {
            self.say(&quote.to_string());
        }
    }

    fn run(&mut self) {
        let mut scheduler = Scheduler::new();
        // TODO: fix multiple clone
        let mut bot = self.clone();
        scheduler
            .every(600.seconds())
            .run(move || bot.endpoint_version_update());
        let bot = self.clone();
        scheduler
            .every(Monday)
            .at("08:00 am")
            .run(move || bot.hello());
        let mut bot = self.clone();
        scheduler
            .every(20.second())
            .run(move || bot.api_online_check());
        loop {
            scheduler.run_pending();
            sleep(Duration::from_millis(100));
        }
    }
}

fn main() {
    let mut bot = match Bot::load() {
        Some(b) => b,
        None => {
            eprintln!("bot requirements are not met. exiting.");
            exit(1);
        }
    };

    if let Err(e) = bot.check() {
        eprintln!("error: {}", e);
        exit(1);
    }
    bot.run()
}
