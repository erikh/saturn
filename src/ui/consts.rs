use lazy_static::lazy_static;
use ratatui::prelude::*;

pub const DAYS: usize = 35;
pub const DAYS_IN_WEEK: usize = 7;

lazy_static! {
    pub static ref DARK_CYAN: Color = Color::Rgb(20, 80, 80);
    pub static ref HEADER_STYLE: Style = Style::default().fg(Color::White).bg(*DARK_CYAN);
    pub static ref CELL_STYLE: Style = Style::default().bg(Color::DarkGray);
    pub static ref TITLE_STYLE: Style = Style::default().fg(Color::White);
}
