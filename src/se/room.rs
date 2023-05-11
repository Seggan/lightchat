use std::collections::HashMap;
use std::sync::Arc;
use reqwest::{Client, Response, StatusCode};

use reqwest_cookie_store::CookieStoreMutex;
use serde_json::Value;
use crate::se::SeError;

pub struct Room {
    client: Client,
    fkey: String,
    user_id: u64,
    room_id: u64,
}

impl Room {
    pub fn new(cookies: Arc<CookieStoreMutex>, fkey: String, user_id: u64, room_id: u64) -> Self {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (compatible; automated;) lightchat/0.1.0")
            .cookie_store(true)
            .cookie_provider(cookies.clone())
            .build()
            .unwrap();
        Self { client, fkey, user_id, room_id }
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
}