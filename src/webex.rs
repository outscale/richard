use crate::request_agent;
use log::{info, trace};
use serde::Deserialize;
use std::error::Error;

#[derive(Clone)]
pub struct WebexAgent {
    auth_header: String,
    room_id: String,
    last_unread_message_date: Option<String>,
}

impl WebexAgent {
    pub fn new(token: String, room_id: String) -> WebexAgent {
        WebexAgent {
            auth_header: format!("Bearer {}", token),
            room_id,
            last_unread_message_date: None,
        }
    }

    pub fn post<T: Into<String>>(&self, url: T) -> ureq::Request {
        let url = url.into();
        let agent = request_agent();
        agent.post(&url).set("Authorization", &self.auth_header)
    }

    pub fn get<T: Into<String>>(&self, url: T) -> ureq::Request {
        let url = url.into();
        let agent = request_agent();
        agent.get(&url).set("Authorization", &self.auth_header)
    }

    pub fn check(&self) -> Result<(), Box<ureq::Error>> {
        trace!("my room {} my token {}", self.room_id, self.auth_header);
        let url = format!(
            "https://webexapis.com/v1/rooms/{}/meetingInfo",
            self.room_id
        );
        let agent = request_agent();
        if let Err(e) = agent
            .get(&url)
            .set("Authorization", &self.auth_header)
            .call()
        {
            info!("checking Webex API: KO");
            return Err(Box::new(e));
        }

        info!("checking Webex API: OK");
        Ok(())
    }

    pub fn say<S: Into<String>>(&self, message: S) -> Result<(), Box<ureq::Error>> {
        self.post("https://webexapis.com/v1/messages")
            .send_json(ureq::json!({
            "roomId": &self.room_id,
            "text": &message.into()
            }))?;
        Ok(())
    }

    pub fn say_markdown<S: Into<String>>(&self, message: S) -> Result<(), Box<ureq::Error>> {
        self.post("https://webexapis.com/v1/messages")
            .send_json(ureq::json!({
            "roomId": &self.room_id,
            "markdown": &message.into()
            }))?;
        Ok(())
    }

    pub fn respond<P, M>(&self, parent: P, message: M) -> Result<(), Box<ureq::Error>>
    where
        P: Into<String>,
        M: Into<String>,
    {
        self.post("https://webexapis.com/v1/messages")
            .send_json(ureq::json!({
            "roomId": &self.room_id,
            "parentId": &parent.into(),
            "text": &message.into()
            }))?;
        Ok(())
    }

    pub fn unread_messages(&mut self) -> Result<WebexMessages, Box<dyn Error>> {
        let url = format!(
            "https://webexapis.com/v1/messages?roomId={}&mentionedPeople=me",
            self.room_id
        );
        let mut res: WebexMessages = self.get(&url).call()?.into_json()?;

        // Sort messages by date
        res.items.sort_by(|a, b| a.created.cmp(&b.created));

        // Filter seen messages
        if let Some(last) = &self.last_unread_message_date {
            res.items.retain(|m| m.created > *last);
        }

        // Update last seen date
        if let Some(m) = res.items.iter().last() {
            let date = Some(m.created.clone());
            if self.last_unread_message_date.is_none() {
                res.items.clear();
            }
            self.last_unread_message_date = date;
        } else if self.last_unread_message_date.is_none() {
            self.last_unread_message_date = Some(String::from("0"));
        }

        Ok(res)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct WebexMessages {
    pub items: Vec<WebexMessage>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WebexMessage {
    pub id: String,
    pub text: String,
    pub created: String,
}
