use crate::request_agent;
use log::{trace, warn};
use std::cmp::min;
use std::error::Error;

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

#[derive(Clone, Debug)]
pub enum EndpointError {
    Code(u16),
    Transport(String),
}

impl EndpointError {
    fn from_ureq(ureq_error: ureq::Error) -> EndpointError {
        match ureq_error {
            ureq::Error::Status(code, _response) => EndpointError::Code(code),
            ureq::Error::Transport(transport) => EndpointError::Transport(transport.to_string()),
        }
    }
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
    pub fn update_version(&mut self) -> Option<String> {
        let version = Some(self.get_version().ok()?);
        let mut ret = None;
        if self.version.is_some() && version != self.version {
            ret = version.clone();
        }
        self.version = version;
        ret
    }

    pub fn get_version(&self) -> Result<String, Box<dyn Error>> {
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
                self.last_error = Some(EndpointError::from_ureq(error));
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

    pub fn update_error_rate(&mut self) -> Option<f32> {
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

    pub fn alive(&mut self) -> Option<String> {
        let response: Option<String> = match self.update_alive() {
            (true, false) => match &self.last_error {
                Some(error) => match error {
                    EndpointError::Code(503) => Some(format!("API on {} has been very properly put in maintenance mode by the wonderful ops team, thanks for your understanding", self.name)),
                    EndpointError::Code(other) => Some(format!("API on {} region is down (error code: {})", self.name, other)),
                    EndpointError::Transport(transport) => Some(format!("API on {} region seems down (transport error: {})", self.name, transport)),
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
