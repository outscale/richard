use crate::utils::request_agent;
use log::{error, info, trace, warn};
use reqwest::StatusCode;
use std::cmp::min;
use std::env::{self, VarError};
use std::error::Error;
use std::fmt::Display;
use tokio::sync::RwLock;
use tokio::time::Duration;

use crate::bot::{
    Message, MessageCtx, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam,
};
use async_trait::async_trait;

const HIGH_ERROR_RATE: f32 = 0.1;
pub struct DownDetectors {
    watch_list: Vec<RwLock<DownDetector>>,
}

#[async_trait]
impl Module for DownDetectors {
    fn name(&self) -> &'static str {
        "down_detectors"
    }

    fn params(&self) -> Vec<ModuleParam> {
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
        ]
    }

    fn variation_durations(&self) -> Vec<Duration> {
        vec![Duration::from_secs(2), Duration::from_secs(2)]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            triggers: Some(vec!["/status".to_string()]),
            ..ModuleCapabilities::default()
        }
    }

    async fn module_offering(&self, _modules: &[ModuleData]) {}

    async fn run(&self, variation: usize) -> Option<Vec<Message>> {
        match variation {
            0 => {
                self.run_error_rate().await;
                None
            }
            1 => self.run_alive().await,
            var => {
                error!("variation {var} is not managed");
                None
            }
        }
    }

    async fn trigger(&self, _message: &str) -> Option<Vec<MessageResponse>> {
        trace!("responding to /status");
        let mut response = String::new();
        for e in self.watch_list.iter() {
            let lock = e.read().await;
            let s = format!(
                "{}: alive={}, error_rate={:.2}\n",
                lock.name, lock.alive, lock.error_rate
            );
            response.push_str(s.as_str());
        }
        Some(vec![response])
    }

    async fn send_message(&self, _messages: &[Message]) {}

    async fn read_message(&self) -> Option<Vec<MessageCtx>> {
        None
    }

    async fn resp_message(&self, _parent: MessageCtx, _message: Message) {}
}

impl DownDetectors {
    pub fn new() -> Result<DownDetectors, VarError> {
        let mut watch_list = Vec::new();
        for i in 0..100 {
            let name = env::var(format!("DOWN_DETECTORS_{}_NAME", i));
            let url = env::var(format!("DOWN_DETECTORS_{}_URL", i));
            match (name, url) {
                (Ok(name), Ok(url)) => {
                    info!("down detector on {} configured", name);
                    let new = DownDetector::new(name, url);
                    watch_list.push(RwLock::new(new));
                }
                _ => break,
            }
        }
        if watch_list.is_empty() {
            warn!("down detectors module enabled bot not configuration provided");
        }
        Ok(DownDetectors { watch_list })
    }

    async fn run_error_rate(&self) {
        for down_detector in self.watch_list.iter() {
            let (name, url) = {
                let lock = down_detector.read().await;
                (lock.name.clone(), lock.url.clone())
            };
            let probe = DownDetector::test_url(&name, &url).await;
            let mut lock = down_detector.write().await;
            if let Some(error_rate) = lock.update_error_rate(probe) {
                if error_rate > HIGH_ERROR_RATE {
                    warn!(
                        "high error rate on {}: {:?}%",
                        lock.name,
                        (error_rate * 100.0) as u32
                    );
                }
            }
        }
    }

    async fn run_alive(&self) -> Option<Vec<Message>> {
        let mut messages = Vec::<Message>::new();
        for down_detector in self.watch_list.iter() {
            let (name, url) = {
                let lock = down_detector.read().await;
                (lock.name.clone(), lock.url.clone())
            };
            let probe = DownDetector::test_url(&name, &url).await;
            let mut lock = down_detector.write().await;
            let alive_change = lock.update_alive(probe);
            if let Some(response) = lock.build_alive_message(alive_change) {
                messages.push(response);
            }
        }
        if messages.is_empty() {
            return None;
        }
        Some(messages)
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

    async fn test_url(name: &str, url: &str) -> Result<(), DownDetectorError> {
        let agent = match request_agent() {
            Ok(agent) => agent,
            Err(err) => {
                trace!("{}: agent init: {}", name, err);
                return Err(DownDetectorError::AgentInit(err.to_string()));
            }
        };

        let response = match agent.get(url).send().await {
            Ok(response) => response,
            Err(err) => {
                trace!("{}: post: {}", name, err);
                return Err(DownDetectorError::from_reqwest(err));
            }
        };

        match response.status() {
            StatusCode::OK => Ok(()),
            bad_code => {
                trace!("{}: {}", name, bad_code);
                Err(DownDetectorError::Code(bad_code.as_u16()))
            }
        }
    }

    fn update_alive(&mut self, probe: Result<(), DownDetectorError>) -> (bool, bool) {
        // Schmitt Trigger based on the number of errors
        // https://en.wikipedia.org/wiki/Schmitt_trigger
        const LOW: u8 = 3;
        const HIGH: u8 = 6;
        const MAX_HIGH: u8 = 10;
        let alive_old = self.alive;
        self.access_failure_cnt = match probe {
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
        if alive_old != self.alive {
            warn!(
                "{}: alive went from {} to {}",
                self.name, alive_old, self.alive
            );
        }
        (alive_old, self.alive)
    }

    fn update_error_rate(&mut self, probe: Result<(), DownDetectorError>) -> Option<f32> {
        // A simple sliding mean, only providing value once sliding window is full.
        const SIZE: f32 = 100.0;
        self.error_rate_acc = match probe {
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

    fn build_alive_message(&self, alive_change: (bool, bool)) -> Option<String> {
        let response = match alive_change {
            (true, false) => match &self.last_error {
                Some(error) => match error {
                    DownDetectorError::AgentInit(err) => {
                        error!("{}", err);
                        None
                    }
                    err => Some(format!("[{}]({}): {}", self.name, self.url, err,)),
                },
                None => Some(format!(
                    "[{}]({}) seems down (no reason found)",
                    self.name, self.url
                )),
            },
            (false, true) => Some(format!("[{}]({}) is up", self.name, self.url)),
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

impl Display for DownDetectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownDetectorError::AgentInit(err) => write!(f, "Internal error: {}", err),
            DownDetectorError::Code(503) => write!(f, "target has been very properly put in maintenance mode by the wonderful ops team, thanks for your understanding"),
            DownDetectorError::Code(other) => write!(f, "target is down (error code: {})", other),
            DownDetectorError::Transport(transport) => write!(f, "target seems down (transport error: {})", transport),
        }
    }
}
