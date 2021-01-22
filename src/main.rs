/* Copyright Outscale SAS */
use clokwerk::{Scheduler, TimeUnits};
use rand::seq::IteratorRandom;
use std::env;
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;
use ureq;

#[derive(Clone)]
struct OscEndpoint {
    name: String,
    endpoint: String,
    version: Option<String>,
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
        let json: serde_json::Value = ureq::post(&self.endpoint).call()?.into_json()?;
        Ok(json["Version"].to_string())
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
        if let Err(e) = ureq::get(&url).set("Authorization", &auth).call() {
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
        let resp = ureq::post("https://webexapis.com/v1/messages")
            .set("Authorization", &auth)
            .send_json(ureq::json!({
            "roomId": &self.room_id,
            "text": &message
            }));
        if let Err(e) = resp {
            eprintln!("error: {}", e);
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
        for message in messages.iter() {
            self.say(message);
        }
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
        let mut bot = self.clone();
        scheduler
            .every(600.seconds())
            .run(move || bot.endpoint_version_update());
        let bot = self.clone();
        scheduler
            .every(1.day())
            .at("08:00 am")
            .run(move || bot.hello());
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
