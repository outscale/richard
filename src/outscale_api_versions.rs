use crate::utils::request_agent;
use log::{debug, error, info, trace, warn};
use serde::Deserialize;
use std::collections::HashMap;
use std::env::{self, VarError};
use std::error::Error;
use tokio::time::Duration;

use crate::bot::{
    Message, MessageCtx, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam,
};
use async_trait::async_trait;

#[derive(Clone, Default)]
pub struct OutscaleApiVersions {
    endpoints: Vec<Endpoint>,
}

#[async_trait]
impl Module for OutscaleApiVersions {
    fn name(&self) -> &'static str {
        "outscale_api_versions"
    }

    fn params(&self) -> Vec<ModuleParam> {
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
        ]
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) -> Option<Vec<Message>> {
        self.run_version().await
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(30)]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            triggers: Some(vec!["/oapi-versions".to_string()]),
            ..ModuleCapabilities::default()
        }
    }

    async fn trigger(&mut self, _message: &str) -> Option<Vec<MessageResponse>> {
        trace!("responding to /oapi-versions");
        let mut response = String::new();
        for endpoint in &self.endpoints {
            let mut versions = endpoint
                .versions
                .keys()
                .cloned()
                .collect::<Vec<String>>()
                .join(", ");
            if versions.is_empty() {
                versions = "unknown".to_string();
            }
            let s = format!("{}: version(s): {}\n", endpoint.name, versions);
            response.push_str(s.as_str());
        }
        Some(vec![response])
    }

    async fn send_message(&mut self, _messages: &[Message]) {}

    async fn read_message(&mut self) -> Option<Vec<MessageCtx>> {
        None
    }

    async fn resp_message(&mut self, _parent: MessageCtx, _message: Message) {}
}

impl OutscaleApiVersions {
    pub fn new() -> Result<OutscaleApiVersions, VarError> {
        let mut endpoints = OutscaleApiVersions::default();
        for i in 0..100 {
            let name = env::var(format!("OUTSCALE_API_VERSIONS_{}_NAME", i));
            let endpoint = env::var(format!("OUTSCALE_API_VERSIONS_{}_ENDPOINT", i));
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

    pub async fn run_version(&mut self) -> Option<Vec<Message>> {
        let mut messages = Vec::<Message>::new();
        for endpoint in self.endpoints.iter_mut() {
            trace!("getting {} endpoint's version", endpoint.name);
            if let Err(err) = endpoint.update_version().await {
                error!(
                    "error while getting endpoint's version {}: {}",
                    endpoint.name, err
                );
            };
            let mut alive_versions = Vec::<String>::new();
            let mut dead_versions = Vec::<String>::new();
            for (version_name, cnt) in endpoint.versions.iter_mut() {
                *cnt = cnt.saturating_sub(1);
                if *cnt == 0 {
                    dead_versions.push(version_name.clone());
                } else {
                    alive_versions.push(version_name.clone());
                }
            }
            messages.push(format!(
                "{}: API version(s) not reachable anymore: {}. Current active version(s): {}. Type /oapi-versions for all details.",
                endpoint.name,
                dead_versions.join(", "),
                alive_versions.join(", ")
            ));
        }
        None
    }
}

#[derive(Clone)]
struct Endpoint {
    name: String,
    endpoint: String,
    // Name -> Counter
    versions: HashMap<String, u8>,
}

impl Endpoint {
    fn new(name: String, endpoint: String) -> Self {
        Endpoint {
            name,
            endpoint,
            versions: HashMap::new(),
        }
    }

    async fn update_version(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let version = self.get_version().await?;
        self.versions.insert(version, 255);
        Ok(())
    }

    async fn get_version(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        let body = request_agent()?
            .post(&self.endpoint)
            .send()
            .await?
            .text()
            .await?;
        let response: VersionResponse = serde_json::from_str(body.as_str())?;
        debug!("endpoint {} read version {}", self.name, response.version);
        Ok(response.version)
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct VersionResponse {
    version: String,
}
