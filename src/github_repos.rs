use crate::bot::{
    Message, MessageCtx, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam,
};
use crate::utils::request_agent;
use async_trait::async_trait;
use chrono::prelude::{DateTime, Utc};
use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::env::VarError;
use std::time::SystemTime;
use tokio::time::Duration;

const DEFAULT_ITEM_PER_PAGE: usize = 60;

pub fn params() -> Vec<ModuleParam> {
    vec![
        ModuleParam::new("GITHUB_TOKEN", "Github token to make api calls", true),
        ModuleParam::new(
            "GITHUB_REPOS_0_FULLNAME",
            "Specific github repo to watch. e.g. kubernetes/kubernetes. Can be multiple (0..)",
            false,
        ),
    ]
}

#[async_trait]
impl Module for GithubRepos {
    fn name(&self) -> &'static str {
        "github_repos"
    }

    fn params(&self) -> Vec<ModuleParam> {
        params()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) -> Option<Vec<Message>> {
        for (_repo_full_name, repo) in self.repos.iter_mut() {
            repo.run().await;
        }
        None
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(3600)]
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities::default()
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

type RepoFullName = String;

#[derive(Clone)]
pub struct GithubRepos {
    repos: HashMap<RepoFullName, GithubRepo>,
}

impl GithubRepos {
    pub fn new() -> Result<Self, VarError> {
        let mut github_repos = GithubRepos {
            repos: HashMap::new(),
        };
        for i in 0..100 {
            let var_fullname = env::var(&format!("GITHUB_REPOS_{}_FULLNAME", i));
            match var_fullname {
                Ok(fullname) => {
                    info!("github repo configured: {}", fullname);
                    let new_repo = GithubRepo::new(fullname.as_str())?;
                    github_repos.repos.insert(fullname, new_repo);
                }
                _ => break,
            }
        }
        if github_repos.repos.is_empty() {
            warn!("github_repos module enabled bot not configuration provided");
        }
        Ok(github_repos)
    }
}

type ReleaseId = String;

#[derive(Clone, Debug, Default)]
pub struct GithubRepo {
    full_name: String,
    details: Option<GithubRepoLight>,
    releases: Option<HashMap<ReleaseId, Release>>,
    github_token: String,
}

impl GithubRepo {
    pub fn new(full_name: &str) -> Result<Self, VarError> {
        let full_name = full_name.into();
        let github_token = env::var("GITHUB_TOKEN")?;
        Ok(GithubRepo {
            full_name,
            github_token,
            ..Default::default()
        })
    }

    pub async fn run(&mut self) -> Option<Vec<Message>> {
        debug!("github_repo: checking for repo {}", self.full_name);
        if self.details.is_none() {
            self.get_repo_details().await;
        }
        match self.is_maintained() {
            Some(true) => {}
            Some(false) => {
                trace!(
                    "repo {} is not maintained, not getting releases",
                    self.full_name
                );
                return None;
            }
            None => {
                trace!("cannot get maintenance details yet for {}", self.full_name);
                return None;
            }
        };
        let Some(current_releases) = self.get_releases().await else {
            error!("no release found for {}", self.full_name);
            return None;
        };

        if self.releases.is_none() {
            trace!(
                "creating initial release mapping for github repo {} with {} releases",
                self.full_name,
                current_releases.len()
            );
            let initial_release_map =
                current_releases
                    .into_iter()
                    .fold(HashMap::new(), |mut map, release| {
                        map.insert(release.id(), release);
                        map
                    });
            self.releases = Some(initial_release_map);
            return None;
        }

        let Some(mut past_releases) = self.releases.take() else {
            return None;
        };
        let mut all_messages = Vec::new();
        for release in current_releases {
            if past_releases
                .insert(release.id(), release.clone())
                .is_none()
                && !release.is_too_old()
            {
                all_messages.push(release.notification_message(&self.full_name));
            }
        }
        self.releases = Some(past_releases);
        if all_messages.is_empty() {
            return None;
        }
        Some(all_messages)
    }

    pub fn is_maintained(&self) -> Option<bool> {
        let details = self.details.as_ref()?;
        Some(!details.fork && !details.archived)
    }

    async fn get_repo_details(&mut self) {
        let Ok(agent) = request_agent() else {
            error!("cannot get request agent");
            return;
        };
        let url = format!("https://api.github.com/repos/{}", self.full_name);
        let resp = match agent
            .get(&url)
            .header("Authorization", &format!("token {}", self.github_token))
            .header("User-Agent", "richard/0.0.0")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
        {
            Ok(res) => res,
            Err(err) => {
                error!("cannot read repo {}: {:#?}", self.full_name, err);
                return;
            }
        };

        let body = match resp.text().await {
            Ok(body) => body,
            Err(e) => {
                error!("cannot get text: {:#?}", e);
                return;
            }
        };

        let repo_light: GithubRepoLight = match serde_json::from_str(&body) {
            Err(err) => {
                error!("cannot deserializing repo {}: {:#?}", self.full_name, err);
                return;
            }
            Ok(body) => body,
        };
        self.details = Some(repo_light);
    }

    async fn get_releases(&self) -> Option<Vec<Release>> {
        debug!("getting all releases for github repo {}", self.full_name);
        let Ok(agent) = request_agent() else {
            return None;
        };
        let url = format!("https://api.github.com/repos/{}/releases", self.full_name);
        let mut page = 1;
        let mut release_list: Vec<Release> = Vec::new();
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
                    error!("cannot get releases: {:#?}:", e);
                    return None;
                }
            };

            let body = match resp.text().await {
                Ok(body) => body,
                Err(e) => {
                    error!("cannot get text: {:#?}", e);
                    return None;
                }
            };

            let mut releases: Vec<Release> = match serde_json::from_str(&body) {
                Err(e) => {
                    error!("cannot deserializing releases: {:#?}", e);
                    return None;
                }
                Ok(body) => body,
            };
            let size = releases.len();

            release_list.append(&mut releases);

            if size < DEFAULT_ITEM_PER_PAGE {
                break;
            }

            page += 1;
        }
        trace!("release list for {}: {:#?}", self.full_name, release_list);
        Some(release_list)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
struct Release {
    html_url: String,
    tag_name: String,
    name: String,
    prerelease: bool,
    draft: bool,
    body: String,
    published_at: Option<String>,
}

impl Release {
    fn notification_message(&self, project_name: &String) -> String {
        format!(
            "👋 Release de [{} {}]({})",
            project_name, self.name, self.html_url
        )
    }

    fn is_too_old(&self) -> bool {
        let Some(published_at) = self.published_at.clone() else {
            return false;
        };
        let Ok(published_date) = DateTime::parse_from_rfc3339(&published_at) else {
            return false;
        };
        let published_date: DateTime<Utc> = published_date.into();
        let now_date = SystemTime::now();
        let now_date: DateTime<Utc> = now_date.into();
        let diff = now_date - published_date;
        if diff.num_days() < 10 {
            return false;
        }
        true
    }

    fn id(&self) -> String {
        self.tag_name.clone()
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct QueryVersions {
    pub event_type: String,
    pub client_payload: QueryVersionsClientPayload,
}

#[derive(Clone, Debug, Serialize)]
pub struct QueryVersionsClientPayload {
    pub versions: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct GithubRepoLight {
    archived: bool,
    fork: bool,
}
