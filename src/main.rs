use std::error::Error;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;

use cli_clipboard::{ClipboardContext, ClipboardProvider};
use cursive::{Cursive, CursiveExt};
use cursive::align::HAlign;
use cursive::event::Key;
use cursive::traits::{Nameable, Resizable};
use cursive::views::{Dialog, EditView, LinearLayout, TextView};
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::app::{App, AppRef, Status};
use crate::se::User;

mod se;
mod app;

fn main() {
    let app = Arc::new(Mutex::new(
        App {
            status: Status::Login,
            clipboard: ClipboardContext::new().unwrap(),
            user: None,
            message: None,
        }
    ));

    let (to_ui, from_event) = channel::<Command>(32);
    let (to_event, from_ui) = channel::<Command>(32);

    let from_event = Arc::new(Mutex::new(from_event));

    let cloned_app = app.clone();
    thread::spawn(move ||
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(event_thread(cloned_app, to_ui, from_ui))
    );

    let mut siv = Cursive::new();

    siv.add_layer(
        Dialog::around(
            LinearLayout::vertical()
                .child(TextView::new("Email:").h_align(HAlign::Center))
                .child(EditView::new().with_name("email").fixed_width(30))
                .child(TextView::new("Password:").h_align(HAlign::Center))
                .child(EditView::new().secret().with_name("password").fixed_width(30))
        )
            .title("Login")
            .button("Login", move |siv| {
                let email = siv.call_on_name("email", |view: &mut EditView| view.get_content()).unwrap();
                let password = siv.call_on_name("password", |view: &mut EditView| view.get_content()).unwrap();

                if email.is_empty() || password.is_empty() {
                    siv.add_layer(Dialog::info("Email and/or password must not be empty"));
                    return;
                }

                to_event.blocking_send(Command::Login {
                    email: (*email).clone(),
                    password: (*password).clone(),
                }).unwrap();

                match from_event.lock().unwrap().blocking_recv().unwrap() {
                    Command::Error(err) => {
                        siv.add_layer(Dialog::info(err.to_string()));
                    }
                    Command::Success => {
                        siv.pop_layer();
                        // TODO
                    }
                    _ => unreachable!(),
                }
            })
    );

    siv.add_global_callback(Key::Esc, |siv| siv.quit());

    siv.run();
}

async fn event_thread(app: AppRef, to_ui: Sender<Command>, mut from_ui: Receiver<Command>) {
    let mut app = app.lock().unwrap();
    while app.status == Status::Login {
        if let Command::Login { email, password } = from_ui.recv().await.unwrap() {
            let mut user = User::new();
            if let Err(error) = user.login(&email, &password).await {
                to_ui.send(Command::Error(Box::new(error))).await.unwrap();
            }
            let room = user.join_room(1).await.unwrap();
            room.send_message(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    .to_string()
                    .as_str()
            ).await.unwrap();
            app.user = Some(user);
            app.status = Status::InRoom;
            to_ui.send(Command::Success).await.unwrap();
        }
    }
}

#[derive(Debug)]
enum Command {
    Login { email: String, password: String },
    Error(Box<dyn Error + Send>),
    Send(String),
    Success,
}
