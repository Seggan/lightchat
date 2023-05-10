use std::collections::HashMap;
use std::future::Future;

use tui::backend::Backend;
use tui::Frame;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::Style;
use tui::widgets::{Block, Borders, BorderType};

use crate::{AppRef, Status};
use crate::input::TextInput;
use crate::se::{SeError, User};

pub fn get_ui<B: Backend>(f: &mut Frame<'_, B>, app: AppRef, fields: &mut HashMap<String, TextInput<'_>>) {
    let app = app.lock().unwrap();
    if app.status == Status::Email {
        do_input_screen("Enter email", f, fields, |app, email| async move {
            let mut user = User::new();
            let result = user.login(
                email.as_str(),
                None,
            ).await;
            let mut app = app.lock().unwrap();
            if let Err(SeError::PasswordRequired) = result {
                app.status = Status::Password(email.to_string());
            } else if let Ok(_) = result {
                app.user = Some(user);
            } else {
                result.unwrap();
            }
        });
    } else if let Status::Password(email) = &app.status {
        let email = email.clone();
        do_input_screen("Enter password", f, fields, |app, password| async move {
            let mut user = User::new();
            let result = user.login(
                &email,
                Some(password.as_str()),
            ).await;
            if let Ok(_) = result {
                let room = user.join_room(1).unwrap();
                room.send_message("Hello, world!").await.unwrap();
                app.lock().unwrap().user = Some(user);
            } else {
                result.unwrap();
            }
        });
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