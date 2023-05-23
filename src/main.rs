use std::collections::HashMap;
use std::fmt::Debug;
use std::io;
use std::sync::{Arc, Mutex};

use cli_clipboard::{ClipboardContext, ClipboardProvider};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, read};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use tui::backend::CrosstermBackend;
use tui::Terminal;
use tui::widgets::ListState;
use tui_textarea::{Input, Key};

use crate::input::TextInput;
use crate::se::{Message, User};
use crate::ui::get_ui;

mod se;
mod input;
mod ui;

#[tokio::main]
async fn main() -> io::Result<()> {
    console_subscriber::init();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;

    let app = Arc::new(Mutex::new(
        App {
            status: Status::Email,
            clipboard: ClipboardContext::new().unwrap(),
            user: None,
            message: None,
            messages_state: ListState::default(),
            messages: Vec::new(),
        }
    ));

    let mut input_fields = HashMap::new();
    loop {
        terminal.draw(|f| get_ui(f, app.clone(), &mut input_fields)).unwrap();
        if let Some(new_fields) = handle_input(input_fields, app.clone()).await {
            input_fields = new_fields;
        } else {
            break;
        }
        let mut app = app.lock().unwrap();
        if app.status == Status::Closing && app.message.is_none() {
            break;
        }
        if let Some(user) = &app.user {
            if let Some(room) = user.current_room() {
                app.messages = room.get_messages().await;
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}

pub const APP_USER_AGENT: &str = concat!(
"Mozilla/5.0 (compatible; automated) ",
concat!(
env!("CARGO_PKG_NAME"),
"/",
env!("CARGO_PKG_VERSION")
)
);

fn key_event_to_input(event: KeyEvent) -> Input {
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    let alt = event.modifiers.contains(KeyModifiers::ALT);
    let key = match event.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter => Key::Enter,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Tab => Key::Tab,
        KeyCode::Delete => Key::Delete,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::F(x) => Key::F(x),
        KeyCode::Esc => Key::Esc,
        _ => Key::Null,
    };
    Input { key, ctrl, alt }
}

/// Returns true if the program should exit
async fn handle_input(fields: HashMap<String, TextInput<'_>>, app_ref: AppRef) -> Option<HashMap<String, TextInput<'_>>> {
    if let Event::Key(key) = read().unwrap() {
        if key.kind != KeyEventKind::Release {
            let input = key_event_to_input(key);

            if let Input { key: Key::Char('c'), ctrl: true, .. } | Input { key: Key::Esc, .. } = input {
                return None;
            }

            let mut app = app_ref.lock().unwrap();
            if app.message.is_some() {
                if let Input { key: Key::Enter, .. } = input {
                    app.message = None;
                }
            } else {
                let messages = app.messages.len();
                let state = &mut app.messages_state;
                if let Some(selected) = state.selected() {
                    match input.key.clone() {
                        Key::Up => {
                            if selected > 0 {
                                state.select(Some(selected - 1));
                            }
                        },
                        Key::Down => {
                            if selected < messages - 1 {
                                state.select(Some(selected + 1));
                            }
                        },
                        _ => {}
                    }
                }
                drop(app);

                let mut new_inputs = HashMap::new();
                for (name, input_field) in fields.into_iter() {
                    if let Some(new_input_field) = input_field
                        .input(input.clone(), app_ref.clone())
                        .await
                    {
                        new_inputs.insert(name.clone(), new_input_field);
                    }
                }
                return Some(new_inputs);
            }
        }
    }
    Some(fields)
}

pub struct App {
    pub status: Status,
    pub clipboard: ClipboardContext,
    pub user: Option<User>,
    pub message: Option<String>,
    pub messages_state: ListState,
    pub messages: Vec<Message>,
}

pub type AppRef = Arc<Mutex<App>>;

impl App {
    pub fn user(&self) -> &User {
        self.user.as_ref().expect("User not logged in")
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum Status {
    Email,
    Password(String),
    InRoom,
    Closing,
}
