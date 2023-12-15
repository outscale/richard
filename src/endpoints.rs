use crate::utils::request_agent;
use crate::webex::{self, WebexAgent};
use log::info;
use log::{error, trace, warn};
use reqwest::StatusCode;
use serde::Deserialize;
use std::cmp::min;
use std::env::{self, VarError};
use std::error::Error;
use tokio::time::Duration;

use crate::bot::{Module, ModuleData, ModuleParam};
use async_trait::async_trait;

const HIGH_ERROR_RATE: f32 = 0.1;

#[derive(Clone)]
pub struct Endpoints {
    webex: WebexAgent,
    endpoints: Vec<Endpoint>,
}

#[async_trait]
impl Module for Endpoints {
    fn name(&self) -> &'static str {
        "endpoints"
    }

    fn params(&self) -> Vec<ModuleParam> {
        [
            webex::params(),
            vec![
                ModuleParam::new(
                    "REGION_0_NAME",
                    "Outscale region name of the endpoints, can be multiple (0..)",
                    false,
                ),
                ModuleParam::new(
                    "REGION_0_ENDPOINT",
                    "Outscale region endpoint, can be multiple (0..)",
                    false,
                ),
            ],
        ]
        .concat()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn has_needed_params(&self) -> bool {
        true
    }

    async fn run(&mut self, variation: usize) {
        match variation {
            0 => self.run_error_rate().await,
            1 => self.run_alive().await,
            2 => self.run_version().await,
            var => error!("variation {var} is not managed"),
        };
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![
            Duration::from_secs(2),
            Duration::from_secs(2),
            Duration::from_secs(600),
        ]
    }

    async fn trigger(&mut self, message: &str, id: &str) {
        if !message.contains("/status") {
            trace!("ignoring message");
            return;
        }
        trace!("responding to /status");
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
        self.webex.respond(&response, id).await;
    }
}

impl Endpoints {
    pub fn new() -> Result<Endpoints, VarError> {
        let mut endpoints = Endpoints {
            endpoints: Vec::new(),
            webex: WebexAgent::new()?,
        };
        for i in 0..100 {
            let name = env::var(&format!("REGION_{}_NAME", i));
            let endpoint = env::var(&format!("REGION_{}_ENDPOINT", i));
            match (name, endpoint) {
                (Ok(name), Ok(endpoint)) => {
                    info!("endpoint {} configured", name);
                    let new = Endpoint::new(name, endpoint);
                    endpoints.endpoints.push(new);
                }
                _ => break,
            }
        }
        Ok(endpoints)
    }

    // TODO: move this as a separate module with 600s of cooldown
    pub async fn run_version(&mut self) {
        let mut messages = Vec::<String>::new();
        for endpoint in self.endpoints.iter_mut() {
            info!("updating {}", endpoint.name);
            if let Some(v) = endpoint.update_version().await {
                messages.push(format!("New API version on {}: {}", endpoint.name, v));
            }
        }
        self.webex.say_messages(messages).await;
    }

    async fn run_error_rate(&mut self) {
        for endpoint in self.endpoints.iter_mut() {
            if let Some(error_rate) = endpoint.update_error_rate().await {
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

    async fn run_alive(&mut self) {
        let mut messages = Vec::<String>::new();
        for endpoint in self.endpoints.iter_mut() {
            if let Some(response) = endpoint.alive().await {
                messages.push(response);
            }
        }
        self.webex.say_messages(messages).await;
    }
}

#[derive(Clone)]
struct Endpoint {
    name: String,
    endpoint: String,
    version: Option<String>,
    alive: bool,
    access_failure_cnt: u8,
    last_error: Option<EndpointError>,
    error_rate_acc: f32,
    error_rate_cnt: u32,
    error_rate: f32,
}

impl Endpoint {
    fn new(name: String, endpoint: String) -> Self {
        Endpoint {
            name,
            endpoint,
            version: None,
            alive: true,
            access_failure_cnt: 0,
            last_error: None,
            error_rate_acc: 0.0,
            error_rate_cnt: 0,
            error_rate: 0.0,
        }
    }

    // return new version if updated
    // return None on first update
    async fn update_version(&mut self) -> Option<String> {
        let version = Some(self.get_version().await.ok()?);
        let mut ret = None;
        if self.version.is_some() && version != self.version {
            ret = version.clone();
        }
        self.version = version;
        ret
    }

    async fn get_version(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        let body = request_agent()?
            .post(&self.endpoint)
            .send()
            .await?
            .text()
            .await?;
        let response: VersionResponse = serde_json::from_str(body.as_str())?;
        Ok(response.version)
    }

    async fn is_alive(&self) -> Result<(), EndpointError> {
        let agent = match request_agent() {
            Ok(agent) => agent,
            Err(err) => return Err(EndpointError::AgentInit(err.to_string())),
        };

        let response = match agent.post(&self.endpoint).send().await {
            Ok(response) => response,
            Err(err) => return Err(EndpointError::from_reqwest(err)),
        };

        match response.status() {
            StatusCode::OK => Ok(()),
            bad_code => Err(EndpointError::Code(bad_code.as_u16())),
        }
    }

    async fn update_alive(&mut self) -> (bool, bool) {
        // Schmitt Trigger based on the number of errors
        // https://en.wikipedia.org/wiki/Schmitt_trigger
        const LOW: u8 = 3;
        const HIGH: u8 = 6;
        const MAX_HIGH: u8 = 10;
        let alive_old = self.alive;
        match self.is_alive().await {
            Ok(_) => self.access_failure_cnt.saturating_sub(1),
            Err(e) => {
                self.last_error = Some(e);
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

    async fn update_error_rate(&mut self) -> Option<f32> {
        // A simple sliding mean, only providing value once sliding window is full.
        const SIZE: f32 = 100.0;
        self.error_rate_acc = match self.get_version().await {
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

    async fn alive(&mut self) -> Option<String> {
        let response: Option<String> = match self.update_alive().await {
            (true, false) => match &self.last_error {
                Some(error) => match error {
                    EndpointError::AgentInit(err) => {
                        error!("{}", err.to_string());
                        None
                    }
                    err => Some(format!("{} region: {}", self.name, err.to_string())),
                },
                None => Some(format!(
                    "API on {} region seems down (no reason found)",
                    self.name
                )),
            },
            (false, true) => Some(format!("API on {} region is up", self.name)),
            _ => None,
        };
        if self.alive {
            trace!("API of {} region is alive", self.name);
        } else {
            warn!("API of {} region is not alive", self.name);
        }
        response
    }
}

#[derive(Clone, Debug)]
enum EndpointError {
    AgentInit(String),
    Code(u16),
    Transport(String),
}

impl EndpointError {
    fn from_reqwest(err: reqwest::Error) -> EndpointError {
        match err.source() {
            Some(e) => EndpointError::Transport(format!("{}, {}", err, e)),
            None => EndpointError::Transport(err.to_string()),
        }
    }
}

impl ToString for EndpointError {
    fn to_string(&self) -> String {
        match self {
            EndpointError::AgentInit(err) => format!("Internal error: {}", err),
            EndpointError::Code(503) => "API has been very properly put in maintenance mode by the wonderful ops team, thanks for your understanding".to_string(),
            EndpointError::Code(other) => format!("API is down (error code: {})", other),
            EndpointError::Transport(transport) => format!("API seems down (transport error: {})", transport),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct VersionResponse {
    version: String,
}
