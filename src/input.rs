use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::pin::Pin;
use tui::style::Style;
use tui_textarea::{Input, Key, TextArea};
use crate::AppRef;

pub struct TextInput<'a> {
    pub area: TextArea<'a>,
    handler: Box<dyn FnOnce(AppRef, String) -> Pin<Box<dyn Future<Output=()> + Send + 'static>> + Send>,
    multiline: bool,
}

impl<'a> TextInput<'a> {
    pub fn new<F>(handler: impl FnOnce(AppRef, String) -> F + Send + 'static) -> TextInput<'a>
        where F: Future<Output=()> + Send + 'static
    {
        let mut area = TextArea::default();
        area.set_cursor_line_style(Style::default());
        TextInput {
            area,
            handler: Box::new(move |app, text| Box::pin(handler(app, text))),
            multiline: false,
        }
    }

    pub fn multiline(mut self) -> TextInput<'a> {
        self.multiline = true;
        self
    }

    pub async fn input(mut self, input: Input, app: AppRef) -> Option<TextInput<'a>> {
        match input {
            Input { key: Key::Char('v'), ctrl: true, .. } => {
                if let Ok(pasted) = cli_clipboard::get_contents() {
                    self.area.insert_str(pasted);
                }
            }
            Input { key: Key::Char('z'), ctrl: true, .. } => {
                self.area.undo();
            }
            Input { key: Key::Char('Z'), ctrl: true, .. } => {
                self.area.redo();
            }
            Input { key: Key::Enter, ctrl, .. } => {
                if ctrl && self.multiline {
                    self.area.insert_newline();
                } else {
                    ((self.handler)(app, self.area.lines().join("\n"))).await;
                    return None;
                }
            }
            _ => { self.area.input(input); }
        }
        Some(self)
    }
}

impl Debug for TextInput<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextInput").finish()
    }
}