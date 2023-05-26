use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use reqwest::{Client, Response, StatusCode};
use reqwest_cookie_store::CookieStoreMutex;
use select::document::Document;
use select::predicate::{Class, Name, Predicate};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::serde_as;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::app::APP_USER_AGENT;
use crate::se::event::{ChatEventType, on_ws_conn};
use crate::se::SeError;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct RoomSpec {
    pub id: u64,
    pub name: String,
}

pub type EventHandlers =
Arc<Mutex<Vec<Box<dyn FnMut(ChatEventType) -> Pin<Box<dyn Future<Output=()> + Send + 'static>> + Send>>>>;

pub struct Room {
    client: Arc<Client>,
    fkey: String,
    user_id: u64,
    room_id: u64,
    messages: Arc<Mutex<Vec<Message>>>,
    event_handlers: EventHandlers,
    task: JoinHandle<()>,
}

impl Room {
    pub async fn new(cookies: Arc<CookieStoreMutex>, fkey: String, user_id: u64, room_id: u64) -> Self {
        let client = Arc::new(Client::builder()
            .user_agent(APP_USER_AGENT)
            .cookie_store(true)
            .cookie_provider(cookies.clone())
            .build()
            .unwrap()
        );
        let event_handlers = Arc::new(Mutex::new(Vec::new()));
        let moved_client = client.clone();
        let moved_fkey = fkey.clone();
        let moved_event_handlers = event_handlers.clone();
        let task = tokio::spawn(async move {
            let client = moved_client;
            loop {
                let response = client.post("https://chat.stackexchange.com/ws-auth")
                    .form(&[("roomid", room_id.to_string()), ("fkey", moved_fkey.clone())])
                    .send()
                    .await
                    .unwrap()
                    .json::<Value>()
                    .await
                    .unwrap();
                if let Value::Object(obj) = response {
                    if let Value::String(url) = &obj["url"] {
                        let url = format!(
                            "{}?l={}",
                            url,
                            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
                        );
                        on_ws_conn(url, room_id, moved_event_handlers.clone()).await.unwrap();
                    }
                }
            }
        });
        let ret = Self {
            client,
            fkey,
            user_id,
            room_id,
            messages: Arc::new(Mutex::new(Vec::new())),
            event_handlers,
            task,
        };
        let messages = ret.messages.clone();
        ret.register_handler(move |event| {
            let messages = messages.clone();
            async move {
                if let Ok(message) = event.try_into() {
                    let mut messages = messages.lock().await;
                    if !messages.contains(&message) {
                        messages.push(message);
                    }
                }
            }
        }).await;
        ret
    }

    pub async fn send_message(&self, msg: &str) -> Result<u64, SeError> {
        let response = self.request(
            format!("https://chat.stackexchange.com/chats/{}/messages/new", self.room_id),
            [("text", msg)].into(),
        )
            .await?
            .json::<Value>()
            .await?;
        Ok(response["id"].as_u64().unwrap())
    }

    pub async fn get_prev_messages(&self, num_messages: usize) -> Result<(), SeError> {
        let response = self.request(
            format!("https://chat.stackexchange.com/chats/{}/events", self.room_id),
            [("mode", "Messages"), ("msgCount", num_messages.to_string().as_str()), ("since", "0")].into(),
        )
            .await?
            .json::<Value>()
            .await?;
        let events = response.as_object().unwrap()["events"].as_array().unwrap();

        let new = events.into_iter()
            .filter_map(|event| serde_json::from_value::<Message>(event.clone()).ok())
            .collect::<Vec<Message>>();

        let mut messages = self.messages.lock().await;
        messages.retain(|msg| !new.contains(msg));
        messages.extend(new);

        Ok(())
    }

    pub async fn get_messages(&self) -> Vec<Message> {
        {
            let messages = self.messages.lock().await;
            if messages.is_empty() {
                drop(messages);
                self.get_prev_messages(100).await.unwrap();
            }
        }
        self.messages.lock().await.clone()
    }

    pub async fn register_handler<F>(&self, mut handler: impl FnMut(ChatEventType) -> F + Send + 'static)
        where F: Future<Output=()> + Send + 'static
    {
        let mut handlers = self.event_handlers.lock().await;
        handlers.push(Box::new(move |event| Box::pin(handler(event))));
    }

    async fn request(&self, url: String, mut params: HashMap<&str, &str>) -> Result<Response, SeError> {
        params.insert("fkey", self.fkey.as_str());
        let response = self.client.post(url)
            .header("Referer", format!("https://chat.stackexchange.com/rooms/{}", self.room_id))
            .form(&params)
            .send()
            .await;
        return if let Ok(res) = response {
            if res.status().is_success() {
                Ok(res)
            } else if res.status() == StatusCode::CONFLICT {
                Err(SeError::RateLimit)
            } else {
                Err(SeError::BadResponse(res.status().as_u16(), res.text().await.unwrap()))
            }
        } else {
            response.map_err(SeError::Reqwest)
        };
    }

    pub async fn leave(self) {
        self.request(
            format!("https://chat.stackexchange.com/chats/leave/{}", self.room_id),
            [].into(),
        ).await.unwrap();
    }

    pub fn get_id(&self) -> u64 {
        self.room_id
    }
}

impl Drop for Room {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[serde_as]
#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "message_id")]
    pub id: u64,
    pub content: String,
    pub user_id: u64,
    pub room_id: u64,
    #[serde(rename = "user_name")]
    pub username: String,
    #[serde(rename = "time_stamp")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub timestamp: Duration,
}

impl TryFrom<ChatEventType> for Message {
    type Error = SeError;

    fn try_from(event: ChatEventType) -> Result<Self, Self::Error> {
        if let ChatEventType::Message { event, content } = event {
            Ok(Message {
                id: event.message_id,
                content,
                user_id: event.user_id,
                room_id: event.room_id,
                username: event.username,
                timestamp: event.timestamp,
            })
        } else {
            Err(SeError::ExpectedMessageEvent(event))
        }
    }
}

impl PartialEq for Message {
    fn eq(&self, other: &Self) -> bool {
        // the second case is for those fake messages that we send when the user sends a message
        self.id == other.id || (self.content == other.content && self.username == other.username)
    }
}