use crate::bot::request_agent;
use log::{trace, warn, error};
use reqwest::StatusCode;
use std::cmp::min;
use std::error::Error;
use serde::Deserialize;

#[derive(Clone, Debug)]
pub enum EndpointError {
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
#[derive(Clone)]
pub struct Endpoint {
    pub name: String,
    pub endpoint: String,
    pub version: Option<String>,
    pub alive: bool,
    pub access_failure_cnt: u8,
    pub last_error: Option<EndpointError>,
    pub error_rate_acc: f32,
    pub error_rate_cnt: u32,
    pub error_rate: f32,
}

impl Endpoint {
    pub fn new(name: String, endpoint: String) -> Self {
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
    pub async fn update_version(&mut self) -> Option<String> {
        let version = Some(self.get_version().await.ok()?);
        let mut ret = None;
        if self.version.is_some() && version != self.version {
            ret = version.clone();
        }
        self.version = version;
        ret
    }

    pub async fn get_version(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        let body = request_agent()?
            .post(&self.endpoint)
            .send().await?
            .text().await?;
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

    pub async fn update_error_rate(&mut self) -> Option<f32> {
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

    pub async fn alive(&mut self) -> Option<String> {
        let response: Option<String> = match self.update_alive().await {
            (true, false) => match &self.last_error {
                Some(error) => match error {
                    EndpointError::AgentInit(err) => {
                        error!("{}", err.to_string());
                        None
                    },
                    err => Some(format!("{} region: {}", self.name, err.to_string())),
                },
                None => Some(format!("API on {} region seems down (no reason found)", self.name)),
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
