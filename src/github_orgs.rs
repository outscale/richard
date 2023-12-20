use crate::bot::{Message, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam};
use crate::github_repos::{self, GithubRepo};
use crate::utils::request_agent;
use crate::webex;
use async_trait::async_trait;
use log::{error, info, trace, warn};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::env::VarError;
use tokio::time::Duration;

const DEFAULT_ITEM_PER_PAGE: usize = 60;

#[async_trait]
impl Module for GithubOrgs {
    fn name(&self) -> &'static str {
        "github_orgs"
    }

    fn params(&self) -> Vec<ModuleParam> {
        [
            webex::params(),
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
            1 => self.update_repo_listing().await,
            _ => {
                error!("bad variation run()");
                return None;
            }
        }
        None
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

    async fn send_message(&mut self, _messages: Vec<String>) {}
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

    async fn run_all_repos(&mut self) {
        for org in self.orgs.iter_mut() {
            trace!("run on org {}...", org.name);
            org.run().await;
        }
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

    async fn run(&mut self) {
        if self.repos.is_empty() {
            self.update_repo_listing().await;
        }
        for (_full_name, repo) in self.repos.iter_mut() {
            repo.run().await;
        }
    }

    async fn update_repo_listing(&mut self) {
        let Ok(repos) = self.get_all_org_repos().await else {
            return;
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

    async fn get_all_org_repos(&mut self) -> Result<Vec<GithubRepoLight>, reqwest::Error> {
        let agent = request_agent()?;
        let url = format!("https://api.github.com/orgs/{}/repos", self.name);
        let mut page = 1;
        let mut repo_listing: Vec<GithubRepoLight> = Vec::new();
        let default_items_per_page = DEFAULT_ITEM_PER_PAGE.to_string();
        loop {
            let page_str = page.to_string();
            let mut params = HashMap::new();
            params.insert("type", "public");
            params.insert("per_page", &default_items_per_page);
            params.insert("page", page_str.as_str());
            let resp = match agent
                .get(&url)
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

            let mut json: Vec<GithubRepoLight> = match serde_json::from_str(&body) {
                Err(e) => {
                    error!("cannot deserializing all repo for {}: {}", self.name, e);
                    trace!("cannot deserializing all repo. Body: {}", body);
                    break;
                }
                Ok(body) => body,
            };

            let size = json.len();

            repo_listing.append(&mut json);

            if size < DEFAULT_ITEM_PER_PAGE {
                break;
            }
            page += 1;
        }
        trace!(
            "get all reprositories from {} organisation: {} found",
            self.name,
            repo_listing.len()
        );
        Ok(repo_listing)
    }
}

#[derive(Deserialize)]
pub struct GithubRepoLight {
    full_name: String,
}
