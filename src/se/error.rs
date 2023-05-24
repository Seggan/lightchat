use thiserror::Error;
use crate::se::event::ChatEventType;

#[derive(Error, Debug)]
pub enum SeError {
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("{0}")]
    Login(String),

    #[error("Bad credentials")]
    BadCredentials,

    #[error("Rate limited")]
    RateLimit,

    #[error("Bad response: {0}: {1}")]
    BadResponse(u16, String),
    
    #[error("Expected message event, got {0:?}")]
    ExpectedMessageEvent(ChatEventType),
}