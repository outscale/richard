use crate::utils::request_agent;
use crate::webex::WebexAgent;
use lazy_static::lazy_static;
use log::trace;
use log::{error, info};
use regex::Regex;
use std::env::VarError;
use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    hash::{Hash, Hasher},
};
pub type ReleaseHash = u64;
use serde::{Deserialize, Serialize};
use std::env;
use std::process::exit;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tokio::time::Duration;
use chrono::prelude::{DateTime, Utc};
use std::time::SystemTime;

const DEFAULT_ITEM_PER_PAGE: usize = 60;
static GITHUB_ORG_NAMES: [&str; 2] = ["outscale", "outscale-dev"];
static GITHUB_SPECIFIC_ORG_NAMES: [&str; 1] = ["kubernetes"];
static GITHUB_SPECIFIC_REPO_NAMES: [&str; 1] = ["kubernetes"];
static GITHUB_ORG_NAME_TRIGGER: &str = "outscale";
static GITHUB_REPO_NAME_TRIGGER: &str = "cluster-api-provider-outscale";

pub async fn run() {
    MODULE.write().await.run().await;
}

pub async fn run_trigger(message: &str, parent_message: &str) {
    MODULE
        .write()
        .await
        .run_trigger(message, parent_message)
        .await
}

lazy_static! {
    static ref MODULE: Arc<RwLock<Github>> = init();
}

fn init() -> Arc<RwLock<Github>> {
    match Github::new() {
        Ok(h) => Arc::new(RwLock::new(h)),
        Err(err) => {
            error!("cannot initialize module, missing var {:#}", err);
            exit(1);
        }
    }
}

#[derive(Clone)]
struct Github {
    webex: WebexAgent,
    token: String,
    releases: HashMap<String, Option<HashSet<ReleaseHash>>>,
}

impl Github {
    fn new() -> Result<Self, VarError> {
        let token = env::var("GITHUB_TOKEN")?;
        Ok(Github {
            webex: WebexAgent::new()?,
            token,
            releases: HashMap::new(),
        })
    }

    async fn run(&mut self) {
        loop {
            self.check_specific_github_release().await;
            self.check_github_release().await;
            sleep(Duration::from_secs(600)).await;
        }
    }

    async fn run_trigger(&mut self, _message: &str, _parent_message: &str) {}

    async fn get_all_repos(&self, org_name: &str) -> Option<Vec<Repo>> {
        let Ok(agent) = request_agent() else {
            return None;
        };
        let url = format!("https://api.github.com/orgs/{}/repos", org_name);
        let mut page = 1;
        let mut results: Vec<Repo> = Vec::new();
        let default_items_per_page = DEFAULT_ITEM_PER_PAGE.to_string();
        loop {
            let page_str = page.to_string();
            let mut params = HashMap::new();
            params.insert("type", "public");
            params.insert("per_page", &default_items_per_page);
            params.insert("page", page_str.as_str());
            let resp = match agent
                .get(&url)
                .header("Authorization", &format!("token {}", self.token))
                .header("User-Agent", "richard/0.0.0")
                .header("Accept", "application/vnd.github+json")
                .form(&params)
                .send()
                .await
            {
                Ok(res) => res,
                Err(e) => {
                    error!("error: cannot listing all repo for {}: {}", org_name, e);
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

            let mut json: Vec<Repo> = match serde_json::from_str(&body) {
                Err(e) => {
                    error!("cannot deserializing all repo for {}: {}", org_name, e);
                    trace!("cannot deserializing all repo. Body: {}", body);
                    break;
                }
                Ok(body) => body,
            };

            let size = json.len();

            results.append(&mut json);

            if size < DEFAULT_ITEM_PER_PAGE {
                break;
            }

            page += 1;
        }
        trace!("get_all_repos from {}: {} found", org_name, results.len());
        Some(results)
    }

    // Trigger specific github action dispatch
    async fn trigger_version_github_action(
        &self,
        org_name: &str,
        repo_name: std::string::String,
        event_type: &str,
        version: &std::string::String,
    ) -> Option<std::string::String> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/dispatches",
            org_name, repo_name
        );
        let Ok(agent) = request_agent() else {
            error!("cannot create agent");
            return None;
        };
        let Ok(json_body) = serde_json::to_string(&QueryVersions {
            event_type: event_type.to_string(),
            client_payload: QueryVersionsClientPayload {
                versions: vec![version.to_string()],
            },
        }) else {
            error!("cannot convert to string QueryVersions");
            return None;
        };
        let req = match agent
            .post(&url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("User-Agent", "richard/0.0.0")
            .header("Accept", "application/vnd.github+json")
            .body(json_body)
            .send()
            .await
        {
            Ok(req) => req,
            Err(e) => {
                error!("error: can not post for {}/{}: {}", org_name, repo_name, e);
                return None;
            }
        };
        if req.status() != 204 {
            error!(
                "post failed for {}/{} {}",
                org_name,
                repo_name,
                req.status()
            );
            return None;
        }

        Some("Trigger has been launched".to_string())
    }
    // Get specific repo
    async fn get_specific_repos(
        &self,
        org_name: &str,
        repo_names: &Vec<std::string::String>,
    ) -> Option<Vec<Repo>> {
        let Ok(agent) = request_agent() else {
            return None;
        };
        let url = format!("https://api.github.com/orgs/{}/repos", org_name);
        let mut page = 1;
        let mut target_repos: Vec<Repo> = Vec::new();
        let mut results: Vec<Repo> = Vec::new();
        let default_items_per_page = DEFAULT_ITEM_PER_PAGE.to_string();

        loop {
            let page_str = page.to_string();
            let mut params = HashMap::new();
            params.insert("type", "public");
            params.insert("per_page", &default_items_per_page);
            params.insert("page", page_str.as_str());
            let resp = match agent
                .get(&url)
                .header("Authorization", &format!("token {}", self.token))
                .header("User-Agent", "richard/0.0.0")
                .header("Accept", "application/vnd.github+json")
                .form(&params)
                .send()
                .await
            {
                Ok(res) => res,
                Err(e) => {
                    error!("error: cannot listing all repo for {}: {}", org_name, e);
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

            let json: Vec<Repo> = match serde_json::from_str(&body) {
                Err(e) => {
                    error!("cannot deserializing all repo for {}: {}", org_name, e);
                    trace!("cannot deserializing all repo. Body: {}", body);
                    break;
                }
                Ok(body) => body,
            };

            for repo in &json {
                let repo_target_name = &repo.name;
                for repo_name in repo_names {
                    if repo_name == repo_target_name {
                        target_repos.push(repo.clone());
                    }
                }
            }
            let size = json.len();
            results.append(&mut target_repos);
            if size < DEFAULT_ITEM_PER_PAGE {
                break;
            }

            page += 1;
        }
        trace!("get_specific_repos: found {} repos", results.len());
        Some(results)
    }

    async fn get_releases(&self, repo_name: &str) -> Option<Vec<Release>> {
        let Ok(agent) = request_agent() else {
            return None;
        };
        let url = format!("https://api.github.com/repos/{}/releases", repo_name);
        let mut page = 1;
        let mut results: Vec<Release> = Vec::new();
        let default_items_per_page = DEFAULT_ITEM_PER_PAGE.to_string();

        loop {
            let page_str = page.to_string();
            let mut params = HashMap::new();
            params.insert("type", "public");
            params.insert("per_page", &default_items_per_page);
            params.insert("page", page_str.as_str());
            let resp = match agent
                .get(&url)
                .header("Authorization", &format!("token {}", self.token))
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

            results.append(&mut releases);

            if size < DEFAULT_ITEM_PER_PAGE {
                break;
            }

            page += 1;
        }
        Some(results)
    }

    async fn check_specific_github_release(&mut self) {
        let mut repo_specific_names: Vec<std::string::String> = Vec::new();
        let mut release_target_name = "v0.0.0".to_string();

        for repo_specific_name in GITHUB_SPECIFIC_REPO_NAMES {
            repo_specific_names.push(repo_specific_name.to_string())
        }
        for org_specific_name in GITHUB_SPECIFIC_ORG_NAMES {
            let repos = match self
                .get_specific_repos(org_specific_name, &repo_specific_names)
                .await
            {
                Some(value) => value,
                None => continue,
            };
            for repo in repos {
                if repo.is_not_maintained() {
                    continue;
                }
                trace!(
                    "retriving latest release for {}/{}",
                    org_specific_name,
                    repo.name
                );
                let name = &repo.full_name;
                match self.get_releases(name).await {
                    None => {
                        if self.releases.get(name).is_some() {
                            continue;
                        }
                        trace!("Add it to the cache");
                        self.releases.insert(name.to_string(), None);
                    }
                    Some(releases) => match self.releases.get_mut(name) {
                        None => {
                            trace!("Got releases and the project wans not in the cache => storing");
                            let mut release_hashs: HashSet<ReleaseHash> = HashSet::new();
                            for release in releases {
                                if release.is_not_official() {
                                    continue;
                                }
                                let release_hash = calculate_hash(&release.name);
                                trace!("Release {:?} Hash {}", &release, &release_hash);
                                release_hashs.insert(release_hash);
                            }
                            self.releases.insert(name.to_string(), Some(release_hashs));
                        }
                        Some(None) => {
                            trace!("Got releases and no release was found before => storing");
                            let mut release_hashs: HashSet<ReleaseHash> = HashSet::new();
                            for release in releases {
                                if release.is_not_official() {
                                    continue;
                                }
                                let release_hash = calculate_hash(&release.name);
                                release_hashs.insert(release_hash);
                            }
                            self.releases.insert(name.to_string(), Some(release_hashs));
                        }
                        Some(Some(previous_releases)) => {
                            for release in releases {
                                if release.is_not_official() {
                                    continue;
                                }
                                let release_hash = calculate_hash(&release.name);
                                if previous_releases.contains(&release_hash) {
                                    continue;
                                }

                                info!("got release for {} with tag {}", name, &release.tag_name);
                                let release_get_notification =
                                    release.get_notification_message(&repo);
                                release_target_name = release.tag_name.clone();
                                previous_releases.insert(release_hash);
                                if !release.is_too_old() {
                                    self.webex.say_markdown(release_get_notification).await;
                                }
                                previous_releases.insert(release_hash);
                            }
                        }
                    },
                }
            }
        }

        for repo_name in repo_specific_names {
            if let Some(value) = Some(&release_target_name) {
                lazy_static! {
                    static ref REG_SEMANTIC_VERSION: Regex =
                        Regex::new(r"^v\d{1,2}.\d{1,2}.\d{1,2}$").unwrap();
                }
                match REG_SEMANTIC_VERSION.is_match(value) {
                    true => trace!("{} has good format", value),
                    false => {
                        trace!("{} has bad format", value);
                        continue;
                    }
                }

                let event_type = "release";
                if value != "v0.0.0" {
                    trace!("Search with {} on {}", repo_name, value);
                    self.trigger_version_github_action(
                        GITHUB_ORG_NAME_TRIGGER,
                        GITHUB_REPO_NAME_TRIGGER.to_string(),
                        event_type,
                        value,
                    )
                    .await;
                }
            }
        }
    }

    async fn check_github_release(&mut self) {
        for org_name in GITHUB_ORG_NAMES {
            info!("retrieving all repos from {}", org_name);

            let repos = match self.get_all_repos(org_name).await {
                Some(value) => value,
                None => continue,
            };
            for repo in repos {
                if repo.is_not_maintained() {
                    continue;
                }
                trace!("retrieving latest release for {}/{}", org_name, repo.name);
                let name = &repo.full_name;
                match self.get_releases(name).await {
                    None => {
                        // Error while retrieving the release
                        if self.releases.get(name).is_some() {
                            continue;
                        }
                        trace!("Add it to the cache");
                        self.releases.insert(name.to_string(), None);
                    }
                    Some(releases) => match self.releases.get_mut(name) {
                        None => {
                            trace!("Got releases and the project was not in the cache => storing");
                            let mut release_hashs: HashSet<ReleaseHash> = HashSet::new();
                            for release in releases {
                                if release.is_not_official() {
                                    continue;
                                }
                                let release_hash = calculate_hash(&release.name);
                                trace!("Release {:?} Hash {}", &release, &release_hash);
                                release_hashs.insert(release_hash);
                            }
                            self.releases.insert(name.to_string(), Some(release_hashs));
                        }
                        Some(None) => {
                            trace!("Got releases and no release was found before => storing");
                            let mut release_hashs: HashSet<ReleaseHash> = HashSet::new();
                            for release in releases {
                                if release.is_not_official() {
                                    continue;
                                }
                                let release_hash = calculate_hash(&release.name);
                                release_hashs.insert(release_hash);
                            }
                            self.releases.insert(name.to_string(), Some(release_hashs));
                        }
                        Some(Some(previous_releases)) => {
                            for release in releases {
                                if release.is_not_official() {
                                    continue;
                                }
                                let release_hash = calculate_hash(&release.name);
                                trace!("Release {:?} Hash {}", &release, &release_hash);
                                if previous_releases.contains(&release_hash) {
                                    continue;
                                }
                                info!("got release for {} with tag {}", name, release.tag_name);
                                previous_releases.insert(release_hash);
                                if !release.is_too_old() {
                                    let message = release.get_notification_message(&repo);
                                    self.webex.say_markdown(message).await;
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct Repo {
    pub name: String,
    pub full_name: String,
    archived: bool,
    fork: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
struct Release {
    html_url: String,
    pub tag_name: String,
    pub name: String,
    pub prerelease: bool,
    pub draft: bool,
    pub body: String,
    published_at: Option<String>,
}

impl Release {
    pub fn get_notification_message(&self, repo: &Repo) -> String {
        format!(
            "👋 Release de [{} {}]({})",
            repo.name, self.tag_name, self.html_url
        )
    }

    pub fn is_not_official(&self) -> bool {
        self.draft || self.prerelease
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
}

impl Repo {
    pub fn is_not_maintained(&self) -> bool {
        self.fork || self.archived
    }
}

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[derive(Clone, Debug, Serialize)]
struct QueryVersions {
    event_type: String,
    client_payload: QueryVersionsClientPayload,
}

#[derive(Clone, Debug, Serialize)]
struct QueryVersionsClientPayload {
    versions: Vec<String>,
}
