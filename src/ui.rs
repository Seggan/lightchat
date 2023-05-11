use std::collections::HashMap;
use std::future::Future;
use std::time::{SystemTime, UNIX_EPOCH};

use tui::backend::Backend;
use tui::Frame;
use tui::layout::{Alignment, Constraint, Direction, Layout};
use tui::style::{Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, BorderType, Paragraph, Wrap};

use crate::{AppRef, Status};
use crate::input::TextInput;
use crate::se::{SeError, User};

pub fn get_ui<B: Backend>(f: &mut Frame<'_, B>, app: AppRef, fields: &mut HashMap<String, TextInput<'_>>) {
    let app = app.lock().unwrap();
    if let Some(message) = &app.message {
        render_message(f, message);
    } else {
        match &app.status {
            Status::Email =>
                do_input_screen("Enter email", f, fields, |app, email| async move {
                    app.lock().unwrap().status = Status::Password(email.to_string());
                }),
            Status::Password(email) => {
                let email = email.clone();
                do_input_screen("Enter password", f, fields, |app, password| async move {
                    let mut user = User::new();
                    let result = user.login(
                        &email,
                        password.as_str(),
                    ).await;
                    match result {
                        Ok(_) => {
                            let room = user.join_room(1).unwrap();
                            room.send_message(
                                SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis()
                                    .to_string()
                                    .as_str()
                            ).await.unwrap();
                            app.lock().unwrap().user = Some(user);
                        }
                        Err(SeError::BadCredentials) => {
                            app.lock().unwrap().message = Some("Bad username and/or password".to_string());
                        }
                        Err(SeError::Login(error)) => {
                            let mut app = app.lock().unwrap();
                            app.message = Some(format!("Login error: {}", error));
                            app.status = Status::Closing;
                        }
                        _ => result.unwrap(),
                    }
                });
            }
            _ => {}
        }
    }
}

fn do_input_screen<'a, B, F>(
    title: &'a str,
    f: &mut Frame<'_, B>,
    fields: &mut HashMap<String, TextInput<'a>>,
    callback: impl FnOnce(AppRef, String) -> F + Send + 'static,
) where B: Backend,
        F: Future<Output=()> + Send + 'static
{
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(40)
        ].as_ref())
        .split(f.size());
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20)
        ].as_ref())
        .split(vert[1]);
    let input = fields.entry(title.to_string()).or_insert_with(|| {
        let mut input = TextInput::new(callback);
        input.area.set_block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default())
        );
        input
    });
    f.render_widget(input.area.widget(), horiz[1]);
}

fn render_message<B: Backend>(f: &mut Frame<B>, message: &str) {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Min(20),
            Constraint::Percentage(40)
        ].as_ref())
        .split(f.size());
    let horz = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(30)
        ].as_ref())
        .split(vert[1]);
    let text = vec![
        Spans::from(message),
        Spans::from("\n"),
        Spans::from(Span::styled(
            "Press enter to continue",
            Style::default().add_modifier(Modifier::RAPID_BLINK),
        )),
    ];
    let paragraph = Paragraph::new(text)
        .block(Block::default())
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, horz[1]);
}