use thiserror::Error;

#[derive(Error, Debug)]
pub enum SeError {
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Password is required")]
    PasswordRequired,

    #[error("{0}")]
    Login(String),

    #[error("Bad credentials")]
    BadCredentials,

    #[error("Rate limited")]
    RateLimit,

    #[error("Bad response: {0}: {1}")]
    BadResponse(u16, String),
}