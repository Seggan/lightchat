use tui::backend::Backend;
use tui::Frame;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::Style;
use tui::widgets::{Block, Borders, BorderType};

use crate::{App, Status};
use crate::input::TextInput;

pub fn get_ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    if app.status == Status::Login {
        do_email_screen(f, app);
    }
}

fn do_email_screen<B: Backend>(f: &mut Frame<B>, app: &mut App) {
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
    let login = app.input_fields.entry("login".to_string()).or_insert_with(|| {
        let mut input = TextInput::new(|_| {
            unimplemented!();
        });
        input.area.set_block(
            Block::default()
                .title("Enter email")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default())
        );
        input
    });
    f.render_widget(login.area.widget(), horiz[1]);
}