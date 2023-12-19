use crate::utils::request_agent;
use crate::webex::{self, WebexAgent};
use log::info;
use log::{error, trace, warn};
use reqwest::StatusCode;
use std::cmp::min;
use std::env::{self, VarError};
use std::error::Error;
use tokio::time::Duration;

use crate::bot::{Module, ModuleCapabilities, ModuleData, ModuleParam};
use async_trait::async_trait;

const HIGH_ERROR_RATE: f32 = 0.1;

#[derive(Clone)]
pub struct DownDetectors {
    webex: WebexAgent,
    watch_list: Vec<DownDetector>,
}

#[async_trait]
impl Module for DownDetectors {
    fn name(&self) -> &'static str {
        "down_detectors"
    }

    fn params(&self) -> Vec<ModuleParam> {
        [
            webex::params(),
            vec![
                ModuleParam::new(
                    "DOWN_DETECTORS_0_NAME",
                    "Friendly name of what is watched, can be multiple (0..)",
                    false,
                ),
                ModuleParam::new(
                    "DOWN_DETECTORS_0_URL",
                    "URL of what is watched, can be multiple (0..)",
                    false,
                ),
            ],
        ]
        .concat()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, variation: usize) {
        match variation {
            0 => self.run_error_rate().await,
            1 => self.run_alive().await,
            var => error!("variation {var} is not managed"),
        };
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(2), Duration::from_secs(2)]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            triggers: Some(vec!["/status".to_string()]),
        }
    }

    async fn trigger(&mut self, _message: &str, id: &str) {
        trace!("responding to /status");
        let mut response = String::new();
        for e in &self.watch_list {
            let s = format!(
                "{}: alive={}, error_rate={}\n",
                e.name, e.alive, e.error_rate
            );
            response.push_str(s.as_str());
        }
        self.webex.respond(&response, id).await;
    }
}

impl DownDetectors {
    pub fn new() -> Result<DownDetectors, VarError> {
        let mut down_detectors = DownDetectors {
            watch_list: Vec::new(),
            webex: WebexAgent::new()?,
        };
        for i in 0..100 {
            let name = env::var(&format!("DOWN_DETECTORS_{}_NAME", i));
            let url = env::var(&format!("DOWN_DETECTORS_{}_URL", i));
            match (name, url) {
                (Ok(name), Ok(url)) => {
                    info!("down detector on {} configured", name);
                    let new = DownDetector::new(name, url);
                    down_detectors.watch_list.push(new);
                }
                _ => break,
            }
        }
        if down_detectors.watch_list.is_empty() {
            warn!("down detectors module enabled bot not configuration provided");
        }
        Ok(down_detectors)
    }

    async fn run_error_rate(&mut self) {
        for down_detector in self.watch_list.iter_mut() {
            if let Some(error_rate) = down_detector.update_error_rate().await {
                if error_rate > HIGH_ERROR_RATE {
                    warn!(
                        "high error rate on {}: {:?}%",
                        down_detector.name,
                        (error_rate * 100.0) as u32
                    );
                }
            }
        }
    }

    async fn run_alive(&mut self) {
        let mut messages = Vec::<String>::new();
        for down_detector in self.watch_list.iter_mut() {
            if let Some(response) = down_detector.alive().await {
                messages.push(response);
            }
        }
        self.webex.say_messages(messages).await;
    }
}

#[derive(Clone)]
struct DownDetector {
    name: String,
    url: String,
    alive: bool,
    access_failure_cnt: u8,
    last_error: Option<DownDetectorError>,
    error_rate_acc: f32,
    error_rate_cnt: u32,
    error_rate: f32,
}

impl DownDetector {
    fn new(name: String, url: String) -> Self {
        DownDetector {
            name,
            url,
            alive: true,
            access_failure_cnt: 0,
            last_error: None,
            error_rate_acc: 0.0,
            error_rate_cnt: 0,
            error_rate: 0.0,
        }
    }

    async fn test_url(&self) -> Result<(), DownDetectorError> {
        let agent = match request_agent() {
            Ok(agent) => agent,
            Err(err) => {
                trace!("{}: agent init: {}", self.name, err);
                return Err(DownDetectorError::AgentInit(err.to_string()));
            }
        };

        let response = match agent.get(&self.url).send().await {
            Ok(response) => response,
            Err(err) => {
                trace!("{}: post: {}", self.name, err);
                return Err(DownDetectorError::from_reqwest(err));
            }
        };

        match response.status() {
            StatusCode::OK => Ok(()),
            bad_code => {
                trace!("{}: {}", self.name, bad_code);
                Err(DownDetectorError::Code(bad_code.as_u16()))
            }
        }
    }

    async fn update_alive(&mut self) -> (bool, bool) {
        // Schmitt Trigger based on the number of errors
        // https://en.wikipedia.org/wiki/Schmitt_trigger
        const LOW: u8 = 3;
        const HIGH: u8 = 6;
        const MAX_HIGH: u8 = 10;
        let alive_old = self.alive;
        match self.test_url().await {
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
        self.error_rate_acc = match self.test_url().await {
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
                    DownDetectorError::AgentInit(err) => {
                        error!("{}", err.to_string());
                        None
                    }
                    err => Some(format!("{}: {}", self.name, err.to_string())),
                },
                None => Some(format!("{} seems down (no reason found)", self.name)),
            },
            (false, true) => Some(format!("{} is up", self.name)),
            _ => None,
        };
        if self.alive {
            trace!("{} is alive", self.name);
        } else {
            warn!("{} is not alive", self.name);
        }
        response
    }
}

#[derive(Clone, Debug)]
enum DownDetectorError {
    AgentInit(String),
    Code(u16),
    Transport(String),
}

impl DownDetectorError {
    fn from_reqwest(err: reqwest::Error) -> DownDetectorError {
        match err.source() {
            Some(e) => DownDetectorError::Transport(format!("{}, {}", err, e)),
            None => DownDetectorError::Transport(err.to_string()),
        }
    }
}

impl ToString for DownDetectorError {
    fn to_string(&self) -> String {
        match self {
            DownDetectorError::AgentInit(err) => format!("Internal error: {}", err),
            DownDetectorError::Code(503) => "API has been very properly put in maintenance mode by the wonderful ops team, thanks for your understanding".to_string(),
            DownDetectorError::Code(other) => format!("API is down (error code: {})", other),
            DownDetectorError::Transport(transport) => format!("API seems down (transport error: {})", transport),
        }
    }
}
