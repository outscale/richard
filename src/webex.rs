use crate::bot::{
    Message, MessageCtx, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam,
};
use crate::utils::request_agent;
use async_trait::async_trait;
use log::{error, trace};
use reqwest::RequestBuilder;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::env::VarError;
use std::error::Error;
use tokio::time::Duration;

#[async_trait]
impl Module for Webex {
    fn name(&self) -> &'static str {
        "webex"
    }

    fn params(&self) -> Vec<ModuleParam> {
        vec![
            ModuleParam::new("WEBEX_TOKEN", "token provided by webex. See how to create a [controller bot](https://developer.webex.com/docs/bots).", true),
            ModuleParam::new("WEBEX_ROOM_ID", "webex room id where to speak", true),
        ]
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) -> Option<Vec<Message>> {
        None
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities {
            send_message: true,
            read_message: true,
            ..ModuleCapabilities::default()
        }
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        vec![Duration::from_secs(10)]
    }

    async fn trigger(&mut self, _message: &str) -> Option<Vec<MessageResponse>> {
        None
    }

    async fn send_message(&mut self, messages: &[Message]) {
        for message in messages {
            self.agent.say(message).await;
        }
    }

    async fn read_message(&mut self) -> Option<Vec<MessageCtx>> {
        let mut unread_messages = Vec::new();
        let messages = self.agent.unread_messages().await.ok()?;
        for message in messages.items {
            unread_messages.push(MessageCtx {
                content: message.text,
                id: message.id,
            })
        }
        if unread_messages.is_empty() {
            return None;
        }
        Some(unread_messages)
    }

    async fn resp_message(&mut self, parent: MessageCtx, message: Message) {
        self.agent.respond(&message, &parent.id).await;
    }
}

#[derive(Clone)]
pub struct Webex {
    agent: WebexAgent,
}

impl Webex {
    pub fn new() -> Result<Self, VarError> {
        Ok(Webex {
            agent: WebexAgent::new()?,
        })
    }
}

#[derive(Clone, Debug, Default)]
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
    fn new() -> Result<WebexAgent, VarError> {
        let webex_token = env::var("WEBEX_TOKEN")?;
        let room_id = env::var("WEBEX_ROOM_ID")?;
        Ok(WebexAgent {
            auth_header: format!("Bearer {}", webex_token),
            room_id,
            last_unread_message_date: None,
        })
    }

    fn post<T: Into<String>, J: Serialize + ?Sized>(
        &self,
        url: T,
        json: &J,
    ) -> Result<RequestBuilder, Box<dyn Error + Send + Sync>> {
        Ok(request_agent()?
            .post(url.into())
            .json(json)
            .header("Authorization", &self.auth_header))
    }

    fn get<T: Into<String>>(&self, url: T) -> Result<RequestBuilder, Box<dyn Error + Send + Sync>> {
        Ok(request_agent()?
            .get(url.into())
            .header("Authorization", &self.auth_header))
    }

    async fn say<S: Into<String>>(&self, message: S) {
        self.say_generic(message, true).await;
    }

    async fn say_generic<S: Into<String>>(&self, message: S, markdown: bool) {
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

    async fn respond(&self, message: &str, parent: &str) {
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

    async fn unread_messages(&mut self) -> Result<WebexMessages, Box<dyn Error + Send + Sync>> {
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
struct WebexMessages {
    items: Vec<WebexMessage>,
}

#[derive(Clone, Debug, Deserialize)]
struct WebexMessage {
    id: String,
    text: String,
    created: String,
}
