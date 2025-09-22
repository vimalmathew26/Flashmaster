use ratatui::style::{Color, Style};
use ratatui::style::Stylize;

pub fn title_style() -> Style { Style::default().fg(Color::Cyan).bold() }
pub fn hint_style() -> Style { Style::default().fg(Color::DarkGray) }
pub fn selected_style() -> Style { Style::default().fg(Color::Yellow).bold() }
pub fn footer_style() -> Style { Style::default().fg(Color::Gray) }
