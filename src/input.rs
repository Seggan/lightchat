use tui::style::Style;
use tui::widgets::{Block, Widget};
use tui_textarea::{Input, Key, TextArea};

use crate::App;

pub struct TextInput<'a> {
    pub area: TextArea<'a>,
    handler: Box<dyn FnMut(&str)>,
    multiline: bool
}

impl<'a> TextInput<'a> {
    pub fn new(handler: impl FnMut(&str) + 'static) -> TextInput<'a> {
        let mut area = TextArea::default();
        area.set_cursor_line_style(Style::default());
        TextInput {
            area,
            handler: Box::new(handler),
            multiline: false
        }
    }

    pub fn multiline(mut self) -> TextInput<'a> {
        self.multiline = true;
        self
    }

    pub fn input(&mut self, input: Input) {
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
                    (self.handler)(self.area.lines().join("\n").as_str());
                }
            }
            _ => { self.area.input(input); }
        }
    }
}