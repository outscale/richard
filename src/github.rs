use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
};

use serde::Deserialize;

use crate::request_agent;

pub type ReleaseHash = u64;

const DEFAULT_ITEM_PER_PAGE: usize = 60;

#[derive(Clone, Debug)]
pub struct Github {
    pub token: String,
    pub releases: HashMap<String, Option<Vec<ReleaseHash>>>,
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
}

impl Github {
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
                    eprintln!("error: cannot listing all repo for {}: {}", org_name, e);
                    return None;
                }
            };

            let mut json: Vec<Repo> = match req.into_json() {
                Err(e) => {
                    eprintln!(
                        "error: cannot deserializing all repo for {}: {}",
                        org_name, e
                    );
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
                    eprintln!(
                        "error: cannot retrieve latest release for {}: {}",
                        repo_name, e
                    );
                    return None;
                }
            };

            let mut releases: Vec<Release> = match req.into_json() {
                Err(e) => {
                    eprintln!(
                        "error: cannot deserializing latest release for {}: {}",
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
