use lazy_static::lazy_static;
use ratatui::prelude::*;

pub const DAYS: usize = 35;
pub const DAYS_IN_WEEK: usize = 7;

lazy_static! {
    pub static ref DARK_CYAN: Color = Color::Rgb(20, 80, 80);
    pub static ref REALLY_DARK_GRAY: Color = Color::Rgb(50, 50, 50);
    pub static ref DARK_GRAY: Color = Color::Rgb(75, 75, 75);
    pub static ref BRIGHT_CYAN: Color = Color::Rgb(60, 240, 240);
    pub static ref HEADER_STYLE: Style = Style::default().fg(Color::White).bg(*DARK_CYAN);
    pub static ref CELL_STYLE: Style = Style::default().bg(*DARK_GRAY);
    pub static ref TODAY_STYLE: Style = Style::default().fg(*BRIGHT_CYAN).bg(*REALLY_DARK_GRAY);
    pub static ref TITLE_STYLE: Style = Style::default().fg(Color::White);
}
