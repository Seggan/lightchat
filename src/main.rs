use std::error::Error;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cli_clipboard::{ClipboardContext, ClipboardProvider};
use cursive::{Cursive, CursiveExt};
use cursive::align::{HAlign, VAlign};
use cursive::event::Key;
use cursive::traits::{Nameable, Resizable};
use cursive::view::ScrollStrategy;
use cursive::views::{Button, Dialog, DummyView, EditView, LinearLayout, ScrollView, TextArea, TextView};
use cursive_async_view::AsyncView;
use cursive_markup::MarkupView;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::sleep;

use crate::app::{App, AppRef, Status};
use crate::se::{Room, User};

mod se;
mod app;

fn main() {
    let app = Arc::new(tokio::sync::Mutex::new(
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

    let mut siv = Cursive::default();

    let moved_app = app.clone();
    let moved_cb_sink = siv.cb_sink().clone();
    let moved_from_event = from_event.clone();
    let moved_to_event = to_event.clone();
    thread::spawn(move ||
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(event_thread(
                moved_app,
                to_ui,
                from_ui,
                moved_to_event,
                moved_from_event,
                moved_cb_sink,
            ))
    );

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
                        room_list(siv, app.clone(), from_event.clone(), to_event.clone());
                    }
                    _ => unreachable!(),
                }
            })
    );

    siv.add_global_callback(Key::Esc, |siv| siv.quit());

    siv.run();
}

async fn event_thread(
    app: AppRef,
    to_ui: Sender<Command>,
    mut from_ui: Receiver<Command>,
    to_event: Sender<Command>,
    from_event: Arc<Mutex<Receiver<Command>>>,
    cb_sink: CbSink,
) {
    {
        let mut app = app.lock().await;
        while app.status == Status::Login {
            if let Some(Command::Login { email, password }) = from_ui.recv().await {
                let mut user = User::new();
                if let Err(error) = user.login(&email, &password).await {
                    to_ui.send(Command::Error(Box::new(error))).await.unwrap();
                } else {
                    app.user = Some(user);
                    app.status = Status::InRoom;
                    to_ui.send(Command::Success).await.unwrap();
                }
            }
        }
    }

    let moved_to_ui = to_ui.clone();
    let moved_cb_sink = cb_sink.clone();
    let moved_app = app.clone();
    tokio::spawn(async move {
        let mut first = true;
        loop {
            let rooms = moved_app.lock().await.user().get_all_rooms().await;
            match rooms {
                Ok(rooms) => {
                    if first {
                        moved_to_ui.send(Command::Success).await.unwrap();
                        sleep(Duration::from_millis(500)).await;
                        first = false;
                    }
                    let moved_app = moved_app.clone();
                    let moved_from_event = from_event.clone();
                    let moved_to_event = to_event.clone();
                    moved_cb_sink.send(Box::new(move |siv| {
                        siv.call_on_name("room_list", |room_list: &mut LinearLayout| {
                            room_list.clear();
                            room_list.add_child(DummyView);
                            for room in rooms.iter() {
                                let moved_app = moved_app.clone();
                                let moved_from_event = moved_from_event.clone();
                                let moved_to_event = moved_to_event.clone();
                                let room_id = room.id;
                                room_list.add_child(Button::new(room.name.clone(), move |siv| {
                                    moved_to_event.blocking_send(Command::Join(room_id)).unwrap();
                                    in_room(
                                        siv,
                                        moved_app.clone(),
                                        moved_from_event.clone(),
                                        moved_to_event.clone(),
                                    );
                                }));
                            }
                        });
                    })).unwrap();
                }
                Err(err) => {
                    if first {
                        moved_to_ui.send(Command::Error(Box::new(err))).await.unwrap();
                        first = false;
                    }
                }
            }
            sleep(Duration::from_secs(30)).await;
        }
    });
    let moved_app = app.clone();
    tokio::spawn(async move {
        let mut last_count = 0;
        let last_room = Arc::new(Mutex::new(None));
        loop {
            let app = moved_app.lock().await;
            let room = app.user().current_room();
            if let Some(room) = room {
                let messages = room.get_messages().await;
                let id = Some(room.get_id());
                drop(app);
                if last_count < messages.len() || *last_room.lock().unwrap() != id {
                    last_count = messages.len();
                    let moved_last_room = last_room.clone();
                    let res = cb_sink.send(Box::new(move |siv| {
                        siv.call_on_name("messages", |msgs: &mut LinearLayout| {
                            moved_last_room.lock().unwrap().replace(id.unwrap());
                            msgs.clear();
                            msgs.add_child(DummyView);
                            for message in messages.iter() {
                                msgs.add_child(MarkupView::html(
                                    format!("{}: {}", message.username, message.content).as_str()
                                ));
                            }
                        });
                    }));
                    if res.is_err() {
                        break;
                    }
                }
            }
        }
    });

    while let Some(command) = from_ui.recv().await {
        match command {
            Command::Send(message) =>
                to_ui.send(
                    match app.lock().await.user().current_room().unwrap().send_message(&message).await {
                        Ok(_) => Command::Success,
                        Err(error) => Command::Error(Box::new(error)),
                    }
                ).await.unwrap(),
            Command::Join(room_id) => {
                let mut app = app.lock().await;
                let mut user = app.user.as_mut().unwrap();
                user.join_room(room_id).await.unwrap();
                user.current_room = Some(room_id);
                app.status = Status::InRoom;
            }
            Command::Success => (),
            x => unreachable!("{:?}", x),
        }
    }
}

type CbSink = cursive::reexports::crossbeam_channel::Sender<Box<dyn FnOnce(&mut Cursive) + Send + 'static>>;

#[derive(Debug)]
enum Command {
    Login { email: String, password: String },
    Error(Box<dyn Error + Send>),
    Success,
    Send(String),
    Join(u64),
}

fn room_list(siv: &mut Cursive, app: AppRef, from_event: Arc<Mutex<Receiver<Command>>>, to_event: Sender<Command>) {
    let moved_from_event = from_event.clone();
    let view = AsyncView::new_with_bg_creator(
        siv,
        move || {
            match moved_from_event.lock().unwrap().blocking_recv().unwrap() {
                Command::Success => Ok(()),
                Command::Error(err) => Err(err.to_string()),
                _ => unreachable!(),
            }
        },
        move |_| ScrollView::new(
            LinearLayout::vertical()
                .child(DummyView)
                .with_name("room_list")
        ),
    );
    siv.add_layer(
        Dialog::around(view)
            .title("Room List")
    );
}

fn in_room(siv: &mut Cursive, app: AppRef, from_event: Arc<Mutex<Receiver<Command>>>, to_event: Sender<Command>) {
    let moved_to_event = to_event.clone();
    siv.add_layer(
        LinearLayout::vertical()
            .child(
                ScrollView::new(
                    LinearLayout::vertical()
                        .child(DummyView)
                        .with_name("messages")
                ).scroll_strategy(ScrollStrategy::StickToBottom)
            )
            .child(DummyView)
            .child(
                LinearLayout::horizontal()
                    .child(
                        TextArea::new()
                            .with_name("message")
                            .min_height(1)
                            .min_width(16)
                    )
                    .child(
                        Button::new("Send", move |siv| {
                            let message = siv.call_on_name(
                                "message",
                                |view: &mut TextArea| view.get_content().to_string(),
                            ).unwrap();
                            if message.is_empty() {
                                return;
                            }
                            moved_to_event.blocking_send(Command::Send(message)).unwrap();
                            match from_event.lock().unwrap().blocking_recv().unwrap() {
                                Command::Error(err) => {
                                    siv.add_layer(Dialog::info(err.to_string()));
                                }
                                Command::Success => {
                                    siv.call_on_name(
                                        "message",
                                        |view: &mut TextArea| view.set_content(""),
                                    ).unwrap();
                                    siv.focus_name("message").unwrap();
                                }
                                _ => {}
                            }
                        })
                    )
            )
    );
    to_event.blocking_send(Command::Success).unwrap();
}
