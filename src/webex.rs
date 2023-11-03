use crate::bot::request_agent;
use log::{info, trace, error};
use serde::Deserialize;
use std::error::Error;
use reqwest::RequestBuilder;
use serde::Serialize;

#[derive(Clone)]
pub struct WebexAgent {
    auth_header: String,
    room_id: String,
    last_unread_message_date: Option<String>,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct WebexQuery {
    room_id: String,
    parent_id: String,
    text: Option<String>,
    markdown: Option<String>,
}

impl WebexAgent {
    pub fn new(token: String, room_id: String) -> WebexAgent {
        WebexAgent {
            auth_header: format!("Bearer {}", token),
            room_id,
            last_unread_message_date: None,
        }
    }

    pub fn post<T: Into<String>, J: Serialize + ?Sized>(&self, url: T, json: &J) -> Result<RequestBuilder, Box<dyn Error + Send + Sync>> {
        Ok(request_agent()?
        .post(url.into())
        .json(json)
        .header("Authorization", &self.auth_header))
        
    }

    pub fn get<T: Into<String>>(&self, url: T) -> Result<RequestBuilder, Box<dyn Error + Send + Sync>> {
        Ok(request_agent()?
        .get(url.into())
        .header("Authorization", &self.auth_header))
    }

    pub async fn check(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        trace!("my room {} my token {}", self.room_id, self.auth_header);
        let url = format!(
            "https://webexapis.com/v1/rooms/{}/meetingInfo",
            self.room_id
        );
        if let Err(e) = self.get(&url)?.send().await
        {
            info!("checking Webex API: KO");
            return Err(Box::new(e));
        }

        info!("checking Webex API: OK");
        Ok(())
    }

    pub async fn say<S: Into<String>>(&self, message: S) {
        self.say_generic(message, false).await;
    }

    pub async fn say_markdown<S: Into<String>>(&self, message: S) {

        self.say_generic(message, true).await;
    }

    pub async fn say_generic<S: Into<String>>(&self, message: S, markdown: bool) {
        let mut request = WebexQuery {
            room_id: self.room_id.clone(),
            ..Default::default()
        };
        match markdown {
            true => request.markdown = Some(message.into()),
            false => request.text = Some(message.into()),
        };

        let Ok(builder) = self.post("https://webexapis.com/v1/messages", &request) else {
            error!("cannot create post request");
            return;
        };
        match builder
            .send()
            .await {
                Ok(resp) => trace!("status: {}, content: {:#?}", resp.status().to_string(), resp.text().await),
                Err(err) => error!("{}", err),
            };
    }

    pub async fn respond<P, M>(&self, parent: P, message: M) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        P: Into<String>,
        M: Into<String>,
    {
        let request = WebexQuery {
            room_id: self.room_id.clone(),
            parent_id: parent.into(),
            text: Some(message.into()),
            ..Default::default()
        };
        self.post("https://webexapis.com/v1/messages", &request)?
            .send()
            .await?;
        Ok(())
    }

    pub async fn unread_messages(&mut self) -> Result<WebexMessages, Box<dyn Error + Send + Sync>> {
        let url = format!(
            "https://webexapis.com/v1/messages?roomId={}&mentionedPeople=me",
            self.room_id
        );
        let body = self.get(url)?.send().await?.text().await?;
        trace!("{}", body);
        let mut res: WebexMessages = serde_json::from_str(body.as_str())?;

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
