use std::sync::Arc;

use cli_clipboard::ClipboardContext;
use tokio::sync::Mutex;

use crate::se::User;

pub struct App {
    pub status: Status,
    pub clipboard: ClipboardContext,
    pub user: Option<User>,
    pub message: Option<String>,
}

pub type AppRef = Arc<Mutex<App>>;

impl App {
    pub fn user(&self) -> &User {
        self.user.as_ref().expect("User not logged in")
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum Status {
    Login,
    InRoom,
    Closing,
}

pub const APP_USER_AGENT: &str = concat!(
"Mozilla/5.0 (compatible; automated) ",
concat!(
env!("CARGO_PKG_NAME"),
"/",
env!("CARGO_PKG_VERSION")
)
);
