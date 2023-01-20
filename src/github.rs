use crate::webex;
use crate::{
    request_agent,
    webex::{WebexAgent, WebexMessage},
    Bot,
};
use lazy_static::lazy_static;
use log::trace;
use log::{error, info};
use regex::Regex;
use serde::Deserialize;
use std::process::exit;
use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    hash::{Hash, Hasher},
};
pub type ReleaseHash = u64;

const DEFAULT_ITEM_PER_PAGE: usize = 60;
static GITHUB_ORG_NAMES: [&str; 2] = ["outscale", "outscale-dev"];
static GITHUB_SPECIFIC_ORG_NAMES: [&str; 1] = ["kubernetes"];
static GITHUB_SPECIFIC_REPO_NAMES: [&str; 1] = ["kubernetes"];
static GITHUB_ORG_NAME_TRIGGER: &str = "outscale";
static GITHUB_REPO_NAME_TRIGGER: &str = "cluster-api-provider-outscale";
static GITHUB_REPO_NAME_WITH_CHANGELOG: &str = "kubernetes";
static GITHUB_URL_WITH_CHANGELOG: &str = "https://relnotes.k8s.io/?releaseVersions=";
#[derive(Clone, Debug)]
pub struct Github {
    pub token: String,
    pub releases: HashMap<String, Option<HashSet<ReleaseHash>>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Repo {
    pub name: String,
    pub full_name: String,
    archived: bool,
    fork: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Release {
    html_url: String,
    pub tag_name: String,
    pub name: String,
    pub prerelease: bool,
    pub draft: bool,
    pub body: String,
}

impl Github {
    pub fn new(github_token: String) -> Self {
        Self {
            token: github_token,
            releases: HashMap::new(),
        }
    }
    pub fn get_all_repos(&self, org_name: &str) -> Option<Vec<Repo>> {
        let agent = request_agent();
        let url = format!("https://api.github.com/orgs/{}/repos", org_name);
        let mut page = 1;
        let mut results: Vec<Repo> = Vec::new();
        loop {
            let req = match agent
                .get(&url)
                .set("Authorization", &format!("token {}", self.token))
                .set("Accept", "application/vnd.github+json")
                .query("type", "public")
                .query("per_page", &DEFAULT_ITEM_PER_PAGE.to_string())
                .query("page", &page.to_string())
                .call()
            {
                Ok(req) => req,
                Err(e) => {
                    error!("error: cannot listing all repo for {}: {}", org_name, e);
                    return None;
                }
            };

            let mut json: Vec<Repo> = match req.into_json() {
                Err(e) => {
                    error!("cannot deserializing all repo for {}: {}", org_name, e);
                    return None;
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

        Some(results)
    }
    // Trigger specific github action dispatch
    pub fn trigger_version_github_action(
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

        let req = match ureq::post(&url)
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Accept", "application/vnd.github+json")
            .send_json(ureq::json!({
                "event_type": event_type,
                "client_payload": {
                    "versions": [ version ],
                }
            })) {
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
    pub fn get_specific_repos(
        &self,
        org_name: &str,
        repo_names: &Vec<std::string::String>,
    ) -> Option<Vec<Repo>> {
        let agent = request_agent();
        let url = format!("https://api.github.com/orgs/{}/repos", org_name);
        let mut page = 1;
        let mut target_repos: Vec<Repo> = Vec::new();
        let mut results: Vec<Repo> = Vec::new();

        loop {
            let req = match agent
                .get(&url)
                .set("Authorization", &format!("token {}", self.token))
                .set("Accept", "application/vnd.github+json")
                .query("type", "public")
                .query("per_page", &DEFAULT_ITEM_PER_PAGE.to_string())
                .query("page", &page.to_string())
                .call()
            {
                Ok(req) => req,
                Err(e) => {
                    error!("error: cannot listing all repo for {}: {}", org_name, e);
                    return None;
                }
            };
            let json: Vec<Repo> = match req.into_json() {
                Err(e) => {
                    error!("cannot deserializing all repo for {}: {}", org_name, e);
                    return None;
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

        Some(results)
    }
    pub fn describe_release(&mut self, m: WebexMessage, bot: Bot) {
        let mut message = m.text.split_whitespace();
        let message_id = m.id;
        if message.next() != Some("describe") {
            bot.respond(
                &message_id,
                "bad format: describe org_name repo_name version",
            );
        }
        let org_specific_name = message.next().unwrap_or_else(|| exit(1));
        let repo_specific_name = message.next().unwrap_or_else(|| exit(1));
        let version = message.next().unwrap_or_else(|| exit(1));
        if repo_specific_name != GITHUB_REPO_NAME_WITH_CHANGELOG {
            let release_body = self
                .get_github_release_body(org_specific_name, repo_specific_name, version)
                .unwrap();
            bot.respond(&message_id, release_body);
        } else {
            let version_url = GITHUB_URL_WITH_CHANGELOG.to_owned() + version;
            bot.respond(&message_id, version_url);
        }
    }
    pub fn get_releases(&self, repo_name: &str) -> Option<Vec<Release>> {
        let agent = request_agent();
        let url = format!("https://api.github.com/repos/{}/releases", repo_name);
        let mut page = 1;
        let mut results: Vec<Release> = Vec::new();
        loop {
            let req = match agent
                .get(&url)
                .set("Authorization", &format!("token {}", self.token))
                .set("Accept", "application/vnd.github+json")
                .query("per_page", &DEFAULT_ITEM_PER_PAGE.to_string())
                .query("page", &page.to_string())
                .call()
            {
                Ok(req) => req,
                Err(e) => {
                    error!("cannot retrieve latest release for {}: {}", repo_name, e);
                    return None;
                }
            };
            let mut releases: Vec<Release> = match req.into_json() {
                Err(e) => {
                    error!(
                        "cannot deserializing latest release for {}: {}",
                        repo_name, e
                    );
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
    // retrieve github body
    pub fn get_github_release_body(
        &mut self,
        org_specific_name: &str,
        repo_specific_name: &str,
        version: &str,
    ) -> Option<String> {
        lazy_static! {
            static ref REG_SEMANTIC_VERSION: Regex =
                Regex::new(r"^v\d{1,2}.\d{1,2}.\d{1,2}$").unwrap();
        }
        match REG_SEMANTIC_VERSION.is_match(version) {
            true => trace!("{} has good format", version),
            false => {
                trace!("{} has bad format", version);
                exit(1);
            }
        }
        let mut repo_specific_names = Vec::new();
        repo_specific_names.push(repo_specific_name.to_string());

        let repos = match self.get_specific_repos(org_specific_name, &repo_specific_names) {
            Some(value) => value,
            None => Vec::new(),
        };
        let mut release_body = "".to_string();
        for repo in repos {
            if repo.is_not_maintained() {
                continue;
            }
            trace!(
                "retrieving latest release for {}/{}",
                org_specific_name,
                repo.name
            );
            let name = &repo.full_name;
            match self.get_releases(name) {
                None => {
                    if self.releases.get(name).is_some() {
                        continue;
                    }
                }
                Some(releases) => {
                    for release in releases {
                        if release.name == version {
                            release_body = release.body.to_owned();
                        }
                    }
                }
            }
        }
        Some(release_body)
    }
    pub fn check_specific_github_release(&mut self, webex_agent: WebexAgent) {
        let mut repo_specific_names: Vec<std::string::String> = Vec::new();
        let mut release_target_name = "v0.0.0".to_string();

        for repo_specific_name in GITHUB_SPECIFIC_REPO_NAMES {
            repo_specific_names.push(repo_specific_name.to_string())
        }
        for org_specific_name in GITHUB_SPECIFIC_ORG_NAMES {
            let repos = match self.get_specific_repos(org_specific_name, &repo_specific_names) {
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
                match self.get_releases(name) {
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
                                let release_hash = calculate_hash(&release);
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
                                let release_hash = calculate_hash(&release);
                                release_hashs.insert(release_hash);
                            }
                            self.releases.insert(name.to_string(), Some(release_hashs));
                        }
                        Some(Some(previous_releases)) => {
                            for release in releases {
                                if release.is_not_official() {
                                    continue;
                                }
                                let release_hash = calculate_hash(&release);
                                if previous_releases.contains(&release_hash) {
                                    continue;
                                }

                                info!("got release for {} with tag {}", name, release.tag_name);
                                let release_get_notification =
                                    release.get_notification_message(&repo);
                                release_target_name = release.tag_name;
                                previous_releases.insert(release_hash);
                                webex_agent.say_markdown(release_get_notification).ok();
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
                        exit(1);
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
                    );
                }
            }
        }
    }
    pub fn check_github_release(&mut self, webex_agent: WebexAgent) {
        for org_name in GITHUB_ORG_NAMES {
            info!("retrieving all repos from {}", org_name);

            let repos = match self.get_all_repos(org_name) {
                Some(value) => value,
                None => continue,
            };
            for repo in repos {
                if repo.is_not_maintained() {
                    continue;
                }
                trace!("retrieving latest release for {}/{}", org_name, repo.name);
                let name = &repo.full_name;
                match self.get_releases(name) {
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
                                let release_hash = calculate_hash(&release);
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
                                let release_hash = calculate_hash(&release);
                                release_hashs.insert(release_hash);
                            }
                            self.releases.insert(name.to_string(), Some(release_hashs));
                        }
                        Some(Some(previous_releases)) => {
                            for release in releases {
                                if release.is_not_official() {
                                    continue;
                                }
                                let release_hash = calculate_hash(&release);
                                trace!("Release {:?} Hash {}", &release, &release_hash);
                                if previous_releases.contains(&release_hash) {
                                    continue;
                                }
                                info!("got release for {} with tag {}", name, release.tag_name);
                                previous_releases.insert(release_hash);
                                webex::WebexAgent::say_markdown(
                                    &webex_agent,
                                    release.get_notification_message(&repo),
                                )
                                .ok();
                            }
                        }
                    },
                }
            }
        }
    }
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

#[cfg(test)]
mod test {
    use crate::load_env;

    use super::*;
    #[test]
    // Check get repo is a success
    fn get_repo_success() {
        let org_specific_name = "kubernetes";
        let mut repo_specific_name = Vec::new();
        repo_specific_name.push("kubernetes".to_string());

        let github_token = load_env("GITHUB_TOKEN");
        let github = Github::new(github_token.unwrap_or_default());
        let repos = match github.get_specific_repos(org_specific_name, &repo_specific_name) {
            Some(value) => value,
            None => Vec::new(),
        };
        for repo in repos {
            assert_eq!(repo.full_name, "kubernetes/kubernetes")
        }
    }
    #[test]
    // Check trigger is a success
    fn get_trigger_success() {
        let event_type = "release";
        let org_specific_name = "outscale";
        let repo_specific_name = "cluster-api-provider-outscale";
        let version = "v1.26.0";
        let github_token = load_env("GITHUB_TOKEN");
        let github = Github::new(github_token.unwrap_or_default());
        let trigger = match github.trigger_version_github_action(
            org_specific_name,
            repo_specific_name.to_owned(),
            event_type,
            &version.to_string(),
        ) {
            Some(value) => value,
            None => "".to_string(),
        };
        assert_eq!(trigger, "Trigger has been launched".to_string())
    }
    #[test]
    // check to retrieve body
    fn get_release_body() {
        let org_specific_name = "outscale";
        let repo_specific_name = "cluster-api-provider-outscale";
        let version = "v0.1.0";
        let github_token = load_env("GITHUB_TOKEN");
        let mut github = Github::new(github_token.unwrap_or_default());
        let release_body =
            match github.get_github_release_body(org_specific_name, repo_specific_name, version) {
                Some(release_body) => release_body,
                None => "no body".to_string(),
            };
        assert_eq!(release_body.contains("Documentation"), true);
    }
}
