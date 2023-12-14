use crate::utils::request_agent;
use log::{error, info, trace};
use reqwest::RequestBuilder;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::env::VarError;
use std::error::Error;

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
    pub fn new() -> Result<WebexAgent, VarError> {
        let webex_token = env::var("WEBEX_TOKEN")?;
        let room_id = env::var("WEBEX_ROOM_ID")?;
        Ok(WebexAgent {
            auth_header: format!("Bearer {}", webex_token),
            room_id,
            last_unread_message_date: None,
        })
    }

    pub fn post<T: Into<String>, J: Serialize + ?Sized>(
        &self,
        url: T,
        json: &J,
    ) -> Result<RequestBuilder, Box<dyn Error + Send + Sync>> {
        Ok(request_agent()?
            .post(url.into())
            .json(json)
            .header("Authorization", &self.auth_header))
    }

    pub fn get<T: Into<String>>(
        &self,
        url: T,
    ) -> Result<RequestBuilder, Box<dyn Error + Send + Sync>> {
        Ok(request_agent()?
            .get(url.into())
            .header("Authorization", &self.auth_header))
    }

    pub async fn check(&self) -> bool {
        trace!("my room {} my token {}", self.room_id, self.auth_header);
        let url = format!(
            "https://webexapis.com/v1/rooms/{}/meetingInfo",
            self.room_id
        );
        let get = match self.get(&url) {
            Ok(get) => get,
            Err(err) => {
                error!("cannot create getter: {:#?}", err);
                return false;
            }
        };
        if let Err(err) = get.send().await {
            error!("webex api: {:#?}", err);
            return false;
        }
        info!("checking Webex API: OK");
        true
    }

    pub async fn say_messages(&self, messages: Vec<String>) {
        for message in messages.iter() {
            self.say(message).await;
        }
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
        match builder.send().await {
            Ok(resp) => trace!(
                "status: {}, content: {:#?}",
                resp.status().to_string(),
                resp.text().await
            ),
            Err(err) => error!("{}", err),
        };
    }

    pub async fn respond(&self, message: &str, parent: &str) {
        trace!("richard responding to parent id {parent}: {message}");
        let request = WebexQuery {
            room_id: self.room_id.clone(),
            parent_id: parent.into(),
            text: Some(message.into()),
            ..Default::default()
        };

        let post = match self.post("https://webexapis.com/v1/messages", &request) {
            Ok(post) => post,
            Err(err) => {
                error!("webex post: {:#?}", err);
                return;
            }
        };
        if let Err(err) = post.send().await {
            error!("webex respond: {:#?}", err)
        }
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
