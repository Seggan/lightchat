use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::serde_as;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{ORIGIN, USER_AGENT};
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;

use crate::APP_USER_AGENT;
use crate::se::EventHandlers;

/*
{"content":"test","event_type":1,"id":141800943,"message_id":63567474,"room_id":1,"room_name":"Sandbox","time_stamp":1684029252,"user_id":526756,"user_name":"Seggan"}
{"content":"test (edit again)","event_type":2,"id":141800944,"message_edits":1,"message_id":63567474,"room_id":1,"room_name":"Sandbox","time_stamp":1684029252,"user_id":526756,"user_name":"Seggan"}
{"event_type":10,"id":141800967,"message_id":63567485,"room_id":1,"room_name":"Sandbox","time_stamp":1684029470,"user_id":526756,"user_name":"Seggan"}
 */

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatEvent {
    pub id: u64,
    pub message_id: u64,
    pub room_id: u64,
    pub room_name: String,
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    #[serde(rename = "time_stamp")]
    pub timestamp: Duration,
    pub user_id: u64,
    #[serde(rename = "user_name")]
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ChatEventType {
    Edit {
        #[serde(flatten)]
        event: ChatEvent,
        message_edits: u64,
        content: String,
    },
    Message {
        #[serde(flatten)]
        event: ChatEvent,
        content: String,
    },
    Delete {
        #[serde(flatten)]
        event: ChatEvent
    },
}

pub async fn on_ws_conn(url: String, room_id: u64, event_handlers: EventHandlers) -> Result<(), Box<dyn std::error::Error>> {
    let room_key = format!("r{}", room_id);
    let mut request = url.into_client_request()?;
    let headers = request.headers_mut();
    headers.insert(ORIGIN, HeaderValue::from_static("https://chat.stackexchange.com"));
    headers.insert(USER_AGENT, HeaderValue::from_static(APP_USER_AGENT));
    let (ws, _) = connect_async(request).await?;
    let (_write, mut read) = ws.split();
    while let Some(message) = read.next().await {
        let message = message?;
        if let Message::Text(message) = message {
            let message = serde_json::from_str::<Value>(&message)?;
            for (key, value) in message.as_object().unwrap() {
                if key == room_key.as_str() {
                    let e = value.as_object().unwrap().get("e");
                    if let Some(e) = e {
                        let e = e.as_array().unwrap();
                        let e = e[0].clone();
                        let event = serde_json::from_value::<ChatEventType>(e.clone());
                        if let Ok(event) = event {
                            let mut handlers = event_handlers.lock().await;
                            for handler in handlers.iter_mut() {
                                handler(event.clone()).await;
                            }
                        } else {
                            panic!("Failed to parse event: {}", e);
                        }
                    }
                    break;
                }
            }
        } else if let Message::Close(_) = message {
            break;
        }
    }
    Ok(())
}