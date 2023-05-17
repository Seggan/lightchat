use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;

use reqwest::{Client, Response, StatusCode};
use reqwest_cookie_store::CookieStoreMutex;
use scraper::{Html, Selector};
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::APP_USER_AGENT;
use crate::se::event::{ChatEventType, on_ws_conn};
use crate::se::SeError;

#[derive(Debug)]
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
    event_handlers: EventHandlers,
    task: JoinHandle<()>,
}

impl Room {
    pub fn new(cookies: Arc<CookieStoreMutex>, fkey: String, user_id: u64, room_id: u64) -> Self {
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
                println!("{:?}", response);
                if let Value::Object(obj) = response {
                    if let Value::String(url) = &obj["url"] {
                        println!("Connecting to {}", url);
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
        Self { client, fkey, user_id, room_id, event_handlers, task }
    }

    pub async fn send_message(&self, msg: &str) -> Result<u64, SeError> {
        let response = self.request(
            format!("https://chat.stackexchange.com/chats/{}/messages/new", self.room_id).as_str(),
            [("text", msg)].into(),
        )
            .await?
            .json::<Value>()
            .await?;
        Ok(response["id"].as_u64().unwrap())
    }

    pub async fn register_handler<F>(&self, mut handler: impl FnMut(ChatEventType) -> F + Send + 'static)
        where F: Future<Output=()> + Send + 'static
    {
        let mut handlers = self.event_handlers.lock().await;
        handlers.push(Box::new(move |event| Box::pin(handler(event))));
    }

    async fn request(&self, url: &str, mut params: HashMap<&str, &str>) -> Result<Response, SeError> {
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

    pub async fn get_all_rooms() -> Result<Vec<RoomSpec>, reqwest::Error> {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (compatible; automated;) lightchat/0.1.0")
            .build()
            .unwrap();
        let mut rooms = Vec::new();
        let mut params = HashMap::new();
        params.insert("tab", "all");
        params.insert("sort", "active");
        params.insert("filter", "");
        params.insert("pageSize", "20");
        params.insert("page", "1");
        let response = client.post("https://chat.stackexchange.com/rooms")
            .form(&params)
            .send()
            .await?
            .text()
            .await?;
        let document = Html::parse_document(response.as_str());
        let selector = Selector::parse(".page-numbers").unwrap();
        let pages = document.select(&selector)
            .filter_map(|page| page.text().collect::<String>().parse::<u64>().ok())
            .max()
            .unwrap();
        for page_num in 1..=pages {
            let mut params = params.clone();
            let string_num = page_num.to_string();
            params.insert("page", string_num.as_str());
            let response = client.post("https://chat.stackexchange.com/rooms")
                .form(&params)
                .send()
                .await?
                .text()
                .await?;
            let document = Html::parse_document(response.as_str());
            let selector = Selector::parse(".room-name > a").unwrap();
            let new_rooms = document.select(&selector)
                .map(|room| {
                    let id = room.value()
                        .attr("href")
                        .unwrap()
                        .split("/")
                        .nth(2)
                        .unwrap()
                        .parse()
                        .unwrap();
                    let name = room.text().collect::<String>();
                    RoomSpec { id, name }
                });
            rooms.extend(new_rooms);
        }
        Ok(rooms)
    }
}

impl Drop for Room {
    fn drop(&mut self) {
        self.task.abort();
        let room_id = self.room_id;
        let fkey = self.fkey.clone();
        let client = self.client.clone();
        tokio::spawn(async move {
            client.post(format!("https://chat.stackexchange.com/chats/leave/{}", room_id))
                .form(&[("fkey", fkey.as_str())])
                .send()
                .await
                .unwrap();
        });
    }
}