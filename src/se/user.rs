use std::collections::HashMap;
use std::sync::Arc;

use reqwest::Client;
use reqwest_cookie_store::CookieStoreMutex;
use scraper::{Html, Selector};

use crate::se::{Room, SeError};
use crate::APP_USER_AGENT;

pub struct User {
    client: Client,
    cookies: Arc<CookieStoreMutex>,
    fkey: Option<String>,
    user_id: Option<u64>,
    rooms: HashMap<u64, Room>,
    pub current_room: Option<u64>,
}

impl User {
    pub fn new() -> Self {
        let cookies = reqwest_cookie_store::CookieStore::default();
        let cookies = Arc::new(CookieStoreMutex::new(cookies));
        let client = Client::builder()
            .user_agent(APP_USER_AGENT)
            .cookie_store(true)
            .cookie_provider(cookies.clone())
            .build()
            .unwrap();
        Self { client, cookies, fkey: None, user_id: None, rooms: HashMap::new(), current_room: None }
    }

    pub async fn login(&mut self, email: &str, password: &str) -> Result<(), SeError> {
        let host = "meta.stackexchange.com"; // Change if bork
        if !self.cookies.lock().unwrap().contains("stackexchange.com", "/", "acct") {
            let fkey = self.get_fkey(format!("https://{}/users/login", host).as_str()).await?;
            let response = self.do_login(email, password, &fkey, host).await?;
            if response != "Login-OK" {
                return Err(SeError::Login(format!("Site login failed: {}", response)));
            }
            self.load_profile(email, password, &fkey, host).await?;
        }

        self.fkey = Some(
            self.get_fkey("https://chat.stackexchange.com/chats/join/favorite")
                .await
                .map_err(|_| SeError::BadCredentials)?
        );
        self.user_id = Some(
            self.get_id()
                .await
                .map_err(|_| SeError::BadCredentials)?
        );
        Ok(())
    }

    pub async fn join_room(&mut self, room_id: u64) -> Result<&Room, SeError> {
        // if we already have the room, just return it
        // have to use this workaround because of https://github.com/rust-lang/rfcs/blob/master/text/2094-nll.md#problem-case-3-conditional-control-flow-across-functions
        if self.rooms.contains_key(&room_id) {
            return Ok(self.rooms.get(&room_id).unwrap());
        }
        if let Some(id) = self.user_id {
            if let Some(fkey) = &self.fkey {
                if self.rooms.is_empty() {
                    self.current_room = Some(room_id);
                }
                let room = Room::new(self.cookies.clone(), fkey.clone(), id, room_id).await;
                self.rooms.insert(room_id, room);
                return Ok(self.rooms.get(&room_id).unwrap());
            }
        }
        Err(SeError::BadCredentials)
    }

    pub fn leave_room(&mut self, room_id: u64) {
        // the Drop impl for Room will take care of actually leaving the room
        self.rooms.remove(&room_id);
    }

    pub fn get_room(&self, room_id: u64) -> Option<&Room> {
        self.rooms.get(&room_id)
    }

    pub fn get_rooms(&self) -> Vec<&Room> {
        self.rooms.values().collect()
    }

    pub fn current_room(&self) -> Option<&Room> {
        if let Some(id) = self.current_room {
            return self.get_room(id);
        }
        None
    }

    async fn do_login(&self, email: &str, password: &str, fkey: &str, host: &str) -> Result<String, reqwest::Error> {
        self.client.post(format!("https://{}/users/login-or-signup/validation/track", host))
            .form(&[
                ("email", email),
                ("password", password),
                ("fkey", fkey),
                ("isSignup", "false"),
                ("isLogin", "true"),
                ("isPassword", "false"),
                ("isAddLogin", "false"),
                ("hasCaptcha", "false"),
                ("ssrc", "head"),
                ("submitButton", "Log in"),
            ])
            .send()
            .await?
            .text()
            .await
    }

    async fn get_fkey(&self, site: &str) -> Result<String, SeError> {
        let page = self.client.get(site)
            .send()
            .await?
            .text()
            .await?;
        let document = Html::parse_document(&page);
        let fkey = document.select(&Selector::parse("input[name=fkey]").unwrap())
            .next()
            .ok_or(SeError::Login(String::from("Failed to get fkey <input>")))?
            .value()
            .attr("value")
            .ok_or(SeError::Login(String::from("Failed to get fkey value")))?
            .parse()
            .unwrap();
        Ok(fkey)
    }

    async fn load_profile(&self, email: &str, password: &str, fkey: &str, host: &str) -> Result<(), SeError> {
        let mut form = HashMap::new();
        form.insert("email", email);
        form.insert("password", password);
        form.insert("fkey", fkey);
        form.insert("ssrc", "head");
        let response = self.client.post(format!("https://{}/users/login", host))
            .form(&form)
            .send()
            .await?
            .text()
            .await?;

        let document = Html::parse_document(&response);
        let captcha = document.select(&Selector::parse("title").unwrap())
            .next()
            .unwrap()
            .text()
            .collect::<String>();
        if captcha.contains("Human verification") {
            return Err(SeError::Login(String::from("Captcha required, wait about 5 minutes")));
        }
        Ok(())
    }

    async fn get_id(&self) -> Result<u64, SeError> {
        let response = self.client.get("https://chat.stackexchange.com/chats/join/favorite")
            .send()
            .await?
            .text()
            .await?;

        let document = Html::parse_document(&response);
        let id_str = document.select(&Selector::parse(".topbar-menu-links").unwrap())
            .next()
            .unwrap()
            .select(&Selector::parse("a").unwrap())
            .next()
            .unwrap()
            .value()
            .attr("href")
            .unwrap();
        let id = id_str
            .split("/")
            .nth(2)
            .unwrap()
            .parse();
        return if let Ok(id) = id {
            Ok(id)
        } else {
            if id_str.contains("login") {
                Err(SeError::BadCredentials)
            } else {
                Err(SeError::Login(format!("Failed to get user id from '{}'", id_str)))
            }
        };
    }
}