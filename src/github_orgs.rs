use crate::bot::{
    Message, MessageCtx, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam,
};
use crate::github_repos::{self, GithubRepo};
use crate::utils::request_agent;
use async_trait::async_trait;
use log::{debug, error, info, trace, warn};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::env::VarError;
use std::error::Error;
use tokio::time::Duration;

const DEFAULT_ITEM_PER_PAGE: usize = 100;

#[async_trait]
impl Module for GithubOrgs {
    fn name(&self) -> &'static str {
        "github_orgs"
    }

    fn params(&self) -> Vec<ModuleParam> {
        [
            github_repos::params(),
            vec![
                ModuleParam::new("GITHUB_TOKEN", "Github token to make api calls", true),
                ModuleParam::new(
                    "GITHUB_ORG_0_NAME",
                    "Github organisation name, can be multiple (0..)",
                    false,
                ),
            ],
        ]
        .concat()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, variation: usize) -> Option<Vec<Message>> {
        match variation {
            0 => self.run_all_repos().await,
            1 => {
                self.update_repo_listing().await;
                None
            }
            _ => {
                error!("bad variation run()");
                None
            }
        }
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities::default()
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        let day_s = 60 * 60 * 24;
        vec![Duration::from_secs(3600), Duration::from_secs(day_s)]
    }

    async fn trigger(&mut self, _message: &str) -> Option<Vec<MessageResponse>> {
        None
    }

    async fn send_message(&mut self, _messages: &[Message]) {}

    async fn read_message(&mut self) -> Option<Vec<MessageCtx>> {
        None
    }

    async fn resp_message(&mut self, _parent: MessageCtx, _message: Message) {}
}
#[derive(Clone)]
pub struct GithubOrgs {
    orgs: Vec<GithubOrg>,
}

impl GithubOrgs {
    pub fn new() -> Result<Self, VarError> {
        let mut orgs = GithubOrgs { orgs: Vec::new() };
        for i in 0..100 {
            let org_name = env::var(&format!("GITHUB_ORG_{}_NAME", i));
            match org_name {
                Ok(org_name) => {
                    info!("github organisation configured: {}", org_name);
                    let new_org = GithubOrg::new(org_name.as_str())?;
                    orgs.orgs.push(new_org);
                }
                _ => break,
            }
        }
        if orgs.orgs.is_empty() {
            warn!("github_orgs module enabled bot not configuration provided");
        }
        Ok(orgs)
    }

    async fn run_all_repos(&mut self) -> Option<Vec<Message>> {
        let mut all_messages = Vec::new();
        for org in self.orgs.iter_mut() {
            trace!("run on org {}...", org.name);
            if let Some(mut messages) = org.run().await {
                all_messages.append(&mut messages);
            }
        }
        if all_messages.is_empty() {
            return None;
        }
        Some(all_messages)
    }

    async fn update_repo_listing(&mut self) {
        for org in self.orgs.iter_mut() {
            trace!("update repo listing for org {}", org.name);
            org.update_repo_listing().await;
        }
    }
}

type RepoFullName = String;

#[derive(Clone)]
struct GithubOrg {
    name: String,
    repos: HashMap<RepoFullName, GithubRepo>,
    github_token: String,
}

impl GithubOrg {
    fn new(org_name: &str) -> Result<Self, VarError> {
        Ok(GithubOrg {
            name: org_name.into(),
            repos: HashMap::new(),
            github_token: env::var("GITHUB_TOKEN")?,
        })
    }

    async fn run(&mut self) -> Option<Vec<Message>> {
        if self.repos.is_empty() {
            self.update_repo_listing().await;
        }
        let mut all_messages = Vec::new();
        for (_full_name, repo) in self.repos.iter_mut() {
            if let Some(mut messages) = repo.run().await {
                all_messages.append(&mut messages);
            }
        }
        if all_messages.is_empty() {
            return None;
        }
        Some(all_messages)
    }

    async fn update_repo_listing(&mut self) {
        let repos = match self.get_all_org_repos().await {
            Ok(repos) => repos,
            Err(err) => {
                error!("cannot fetch all org repos: {:#?}", err);
                return;
            }
        };
        for repo in repos {
            let full_name = repo.full_name;
            let repo = match GithubRepo::new(full_name.as_str()) {
                Ok(repo) => repo,
                Err(err) => {
                    error!("cannot create GithubRepo: {:#?}", err);
                    continue;
                }
            };
            self.repos.insert(full_name, repo);
        }
    }

    async fn get_all_org_repos(&mut self) -> Result<Vec<GithubRepoLight>, Box<dyn Error>> {
        debug!("fetching all repos for {} organization", self.name);
        let mut page = 1;
        let mut repo_listing: Vec<GithubRepoLight> = Vec::new();
        let default_items_per_page = DEFAULT_ITEM_PER_PAGE.to_string();
        loop {
            trace!("fetching {} org repos: page {}", self.name, page);
            let page_str = page.to_string();
            let mut params = HashMap::new();
            params.insert("type", "public");
            params.insert("per_page", &default_items_per_page);
            params.insert("page", page_str.as_str());
            params.insert("sort", "full_name");
            let url = format!("https://api.github.com/orgs/{}/repos", self.name);
            let url = reqwest::Url::parse_with_params(&url, &params)?;
            let agent = request_agent()?;
            let resp = match agent
                .get(url.as_str())
                .header("Authorization", &format!("token {}", self.github_token))
                .header("User-Agent", "richard/0.0.0")
                .header("Accept", "application/vnd.github+json")
                .form(&params)
                .send()
                .await
            {
                Ok(res) => res,
                Err(e) => {
                    error!("error: cannot listing all repo for {}: {}", self.name, e);
                    break;
                }
            };

            let body = match resp.text().await {
                Ok(body) => body,
                Err(e) => {
                    error!("cannot get text: {:#?}", e);
                    break;
                }
            };

            let mut fetched_repos: Vec<GithubRepoLight> = match serde_json::from_str(&body) {
                Err(e) => {
                    error!("cannot deserializing all repo for {}: {}", self.name, e);
                    trace!("cannot deserializing all repo. Body: {}", body);
                    break;
                }
                Ok(body) => body,
            };

            let size = fetched_repos.len();
            trace!(
                "{} org: fetched {} repos in this page: {:#?}",
                self.name,
                size,
                fetched_repos
                    .iter()
                    .map(|r| &r.full_name)
                    .collect::<Vec::<&String>>()
            );

            repo_listing.append(&mut fetched_repos);

            if size < DEFAULT_ITEM_PER_PAGE {
                break;
            }
            page += 1;
        }
        debug!(
            "get all reprositories from {} organisation: {} found",
            self.name,
            repo_listing.len()
        );
        Ok(repo_listing)
    }
}

#[derive(Deserialize, Debug)]
pub struct GithubRepoLight {
    full_name: String,
}
