/*
{"content":"test","event_type":1,"id":141800943,"message_id":63567474,"room_id":1,"room_name":"Sandbox","time_stamp":1684029252,"user_id":526756,"user_name":"Seggan"}
{"content":"test (edit again)","event_type":2,"id":141800944,"message_edits":1,"message_id":63567474,"room_id":1,"room_name":"Sandbox","time_stamp":1684029252,"user_id":526756,"user_name":"Seggan"}
{"event_type":10,"id":141800967,"message_id":63567485,"room_id":1,"room_name":"Sandbox","time_stamp":1684029470,"user_id":526756,"user_name":"Seggan"}
 */

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatEvent {
    pub id: u64,
    pub message_id: u64,
    pub room_id: u64,
    pub room_name: String,
    pub time_stamp: u64,
    pub user_id: u64,
    pub user_name: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ChatEventType {
    Edit {
        #[serde(flatten)]
        event: ChatEvent,
        message_edits: u64,
        content: String
    },
    Message {
        #[serde(flatten)]
        event: ChatEvent,
        content: String
    },
    Delete {
        #[serde(flatten)]
        event: ChatEvent
    },
}