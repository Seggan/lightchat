use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::sync::Arc;

use reqwest::Client;
use reqwest_cookie_store::CookieStoreMutex;
use scraper::{Html, Selector};

use crate::se::{Room, SeError};

pub struct User {
    client: Client,
    cookies: Arc<CookieStoreMutex>,
    fkey: Option<String>,
    user_id: Option<u64>,
    rooms: HashMap<u64, Room>
}

impl User {
    pub fn new() -> Self {
        let cookie_file = File::open("cookies.json")
            .map(BufReader::new);
        let cookies = if let Ok(cookie_file) = cookie_file {
            reqwest_cookie_store::CookieStore::load_json(cookie_file).unwrap()
        } else {
            reqwest_cookie_store::CookieStore::default()
        };
        let cookies = Arc::new(CookieStoreMutex::new(cookies));
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (compatible; automated;) lightchat/0.1.0")
            .cookie_store(true)
            .cookie_provider(cookies.clone())
            .build()
            .unwrap();
        Self { client, cookies, fkey: None, user_id: None, rooms: HashMap::new() }
    }

    pub async fn login(&mut self, email: &str, password: Option<&str>, host: &str) -> Result<(), SeError> {
        if !self.cookies.lock().unwrap().contains("stackexchange.com", "/", "acct") {
            let password = if let Some(password) = password {
                password
            } else {
                return Err(SeError::PasswordRequired);
            };
            let fkey = self.get_fkey("https://meta.stackexchange.com/users/login").await?;
            let response = self.do_login(email, password, &fkey, host).await?;
            if response != "Login-OK" {
                return Err(SeError::Login(format!("Site login failed: {}", response)));
            }
            self.load_profile(email, password, &fkey, host).await?;

            let mut cookie_file = BufWriter::new(File::create("cookies.json").unwrap());
            self.cookies.lock().unwrap().save_json(&mut cookie_file).unwrap();
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

    pub fn join_room(&mut self, room_id: u64) -> Result<&Room, SeError> {
        if let Some(id) = self.user_id {
            if let Some(fkey) = &self.fkey {
                let room = Room::new(self.cookies.clone(), fkey.clone(), id, room_id);
                self.rooms.insert(room_id, room);
                return Ok(self.rooms.get(&room_id).unwrap());
            }
        }
        Err(SeError::BadCredentials)
    }

    async fn do_login(&self, email: &str, password: &str, fkey: &str, host: &str) -> Result<String, reqwest::Error> {
        let mut form = HashMap::new();
        form.insert("email", email);
        form.insert("password", password);
        form.insert("fkey", fkey);
        form.insert("isSignup", "false");
        form.insert("isLogin", "true");
        form.insert("isPassword", "false");
        form.insert("isAddLogin", "false");
        form.insert("hasCaptcha", "false");
        form.insert("ssrc", "head");
        form.insert("submitButton", "Log in");
        self.client.post(format!("https://{}/users/login-or-signup/validation/track", host))
            .form(&form)
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

    async fn get_id(&self) -> Result<u64, reqwest::Error> {
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
            .unwrap()
            .split("/")
            .nth(2)
            .unwrap();
        Ok(id_str.parse().unwrap())
    }
}