use crate::utils::request_agent;
use crate::webex::{self, WebexAgent};
use log::{info, trace, warn};
use serde::Deserialize;
use std::env::{self, VarError};
use std::error::Error;
use tokio::time::Duration;

use crate::bot::{Module, ModuleCapabilities, ModuleData, ModuleParam};
use async_trait::async_trait;

#[derive(Clone)]
pub struct OutscaleApiVersions {
    webex: WebexAgent,
    endpoints: Vec<Endpoint>,
}

#[async_trait]
impl Module for OutscaleApiVersions {
    fn name(&self) -> &'static str {
        "outscale_api_versions"
    }

    fn params(&self) -> Vec<ModuleParam> {
        [
            webex::params(),
            vec![
                ModuleParam::new(
                    "OUTSCALE_API_VERSIONS_0_NAME",
                    "Outscale region name of the endpoint, can be multiple (0..)",
                    false,
                ),
                ModuleParam::new(
                    "OUTSCALE_API_VERSIONS_0_ENDPOINT",
                    "Outscale region endpoint, can be multiple (0..)",
                    false,
                ),
            ],
        ]
        .concat()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) {
        self.run_version().await;
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(600)]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            triggers: Some(vec!["/oapi-versions".to_string()]),
            catch_non_triggered: false,
            catch_all: false,
        }
    }

    async fn trigger(&mut self, _message: &str, id: &str) {
        trace!("responding to /status");
        let mut response = String::new();
        for e in &self.endpoints {
            let version = match &e.version {
                Some(v) => v.clone(),
                None => "unkown".to_string(),
            };
            let s = format!("{}: version={}\n", e.name, version);
            response.push_str(s.as_str());
        }
        self.webex.respond(&response, id).await;
    }
}

impl OutscaleApiVersions {
    pub fn new() -> Result<OutscaleApiVersions, VarError> {
        let mut endpoints = OutscaleApiVersions {
            endpoints: Vec::new(),
            webex: WebexAgent::new()?,
        };
        for i in 0..100 {
            let name = env::var(&format!("OUTSCALE_API_VERSIONS_{}_NAME", i));
            let endpoint = env::var(&format!("OUTSCALE_API_VERSIONS_{}_ENDPOINT", i));
            match (name, endpoint) {
                (Ok(name), Ok(endpoint)) => {
                    info!("outscale api version on {} is configured", name);
                    let new = Endpoint::new(name, endpoint);
                    endpoints.endpoints.push(new);
                }
                _ => break,
            }
        }
        if endpoints.endpoints.is_empty() {
            warn!("outscale_api_version module enabled bot not configuration provided");
        }
        Ok(endpoints)
    }

    pub async fn run_version(&mut self) {
        let mut messages = Vec::<String>::new();
        for endpoint in self.endpoints.iter_mut() {
            trace!("updating {} version", endpoint.name);
            if let Some(v) = endpoint.update_version().await {
                messages.push(format!("New API version on {}: {}", endpoint.name, v));
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
}

impl Endpoint {
    fn new(name: String, endpoint: String) -> Self {
        Endpoint {
            name,
            endpoint,
            version: None,
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
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct VersionResponse {
    version: String,
}
