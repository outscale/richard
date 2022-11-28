use crate::request_agent;
use feed_rs::model;
use feed_rs::parser::parse;
use std::cmp::Ordering;
use std::error::Error;

#[derive(Clone)]
pub struct Feed {
    pub name: String,
    pub url: String,
    pub latest: Option<model::Entry>,
}

impl Feed {
    pub fn new(name: String, url: String) -> Self {
        Feed {
            name,
            url,
            latest: None,
        }
    }

    pub fn update(&mut self) -> bool {
        let new_entry = self.last_entry();
        let changed = match (&self.latest, &new_entry) {
            (None, None) => false,
            (Some(old), Some(new)) => old.id != new.id,
            (None, Some(_)) => false,
            (Some(_), None) => false,
        };
        if changed {
            self.latest = new_entry;
        }
        changed
    }

    fn download(&self) -> Result<model::Feed, Box<dyn Error>> {
        println!("downloading feeds for {}", self.name);
        let response = match request_agent().get(&self.url).call() {
            Ok(resp) => resp,
            Err(err) => {
                eprintln!("error: cannot read feed located on {}: {}", self.url, err);
                return Err(Box::new(err));
            }
        };
        match parse(response.into_reader()) {
            Ok(feed) => Ok(feed),
            Err(error) => Err(Box::new(error)),
        }
    }

    fn last_entry(&self) -> Option<model::Entry> {
        let mut feed = self.download().ok()?;
        feed.entries.sort_by(|a, b| {
            if let Some(date_a) = a.published {
                if let Some(date_b) = b.published {
                    return date_b.cmp(&date_a);
                }
            }
            Ordering::Equal
        });
        let entry = feed.entries.first()?;
        Some(entry.clone())
    }

    pub fn announce(&self) -> Option<String> {
        let entry = self.latest.clone()?;
        let title = entry.title.map(|title| title.content);
        let url = entry.links.first().map(|link| link.href.clone());
        Some(match (title, url) {
            (None, None) => format!("New post on [{}]({})", self.name, self.url),
            (None, Some(url)) => format!("New post on [{}]({})", self.name, url),
            (Some(title), None) => format!("New post on [{}]({}): {}", self.name, self.url, title),
            (Some(title), Some(url)) => format!("New post on {}: [{}]({})", self.name, title, url),
        })
    }
}
