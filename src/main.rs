use std::collections::HashMap;
use std::io;
use cli_clipboard::{ClipboardContext, ClipboardProvider};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, read};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use tui::backend::CrosstermBackend;
use tui::Terminal;
use tui_textarea::{Input, Key};
use crate::input::TextInput;

use crate::ui::get_ui;

mod se;
mod ui;
mod input;

#[tokio::main]
async fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;

    let mut app = App {
        input_fields: HashMap::new(),
        status: Status::Login,
        clipboard: ClipboardContext::new().unwrap()
    };
    loop {
        terminal.draw(|f| get_ui(f, &mut app))?;
        if handle_input(&mut app)? {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

pub struct App<'a> {
    pub input_fields: HashMap<String, TextInput<'a>>,
    pub status: Status,
    pub clipboard: ClipboardContext
}

#[derive(PartialEq)]
pub enum Status {
    Login,
}

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
fn handle_input(app: &mut App) -> io::Result<bool> {
    if let Event::Key(key) = read()? {
        if key.kind != KeyEventKind::Release {
            let input = key_event_to_input(key);
            for inp in app.input_fields.values_mut() {
                let input = input.clone();
                if let Input { key: Key::Char('c'), ctrl: true, .. } | Input { key: Key::Esc, .. } = input {
                    return Ok(true);
                }
                inp.input(input);
            }
        }
    }
    Ok(false)
}
