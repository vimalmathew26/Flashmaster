use crate::tui::theme::*;
use flashmaster_core::{Card, Deck};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub enum RightPane<'a> {
    Idle,
    Card { card: &'a Card, reveal: bool },
    Empty(&'a str),
}

pub fn draw_ui(f: &mut Frame, area: Rect, decks: &[Deck], sel: usize, right: RightPane) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);
    draw_decks(f, chunks[0], decks, sel);
    draw_right(f, chunks[1], right);

    let foot = Paragraph::new(Line::from(vec![
        Span::raw(" ↑/k ↓/j select  "),
        Span::raw(" Enter start  "),
        Span::raw(" space reveal  "),
        Span::raw(" 1/2/3 grade  "),
        Span::raw(" s skip  "),
        Span::raw(" q quit "),
    ]))
    .style(footer_style())
    .block(Block::default().borders(Borders::TOP));
    let fh = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };
    f.render_widget(foot, fh);
}

fn draw_decks(f: &mut Frame, area: Rect, decks: &[Deck], sel: usize) {
    let items: Vec<_> = decks
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let s = if i == sel {
                Line::from(d.name.clone()).style(selected_style())
            } else {
                Line::from(d.name.clone())
            };
            ListItem::new(s)
        })
        .collect();

    let title = Paragraph::new(Line::from(vec![Span::raw("Decks").style(title_style())]));
    let th = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    f.render_widget(title, th);

    let list_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height.saturating_sub(1),
    };
    let list = List::new(items).block(Block::default().borders(Borders::ALL));
    f.render_widget(list, list_area);
}

fn draw_right(f: &mut Frame, area: Rect, pane: RightPane) {
    match pane {
        RightPane::Idle => {
            let p = Paragraph::new("Press Enter to start reviewing the selected deck.")
                .wrap(Wrap { trim: true })
                .block(Block::default().title("Review").borders(Borders::ALL));
            f.render_widget(p, area);
        }
        RightPane::Empty(msg) => {
            let p = Paragraph::new(msg)
                .wrap(Wrap { trim: true })
                .block(Block::default().title("Review").borders(Borders::ALL));
            f.render_widget(p, area);
        }
        RightPane::Card { card, reveal } => {
            let title = Block::default().title("Review").borders(Borders::ALL);
            let inner = Rect {
                x: area.x + 1,
                y: area.y + 1,
                width: area.width.saturating_sub(2),
                height: area.height.saturating_sub(2),
            };
            f.render_widget(title, area);

            let q = Paragraph::new(Line::from(vec![
                Span::raw("Q: ").style(title_style()),
                Span::raw(&card.front),
            ]))
            .wrap(Wrap { trim: true });
            f.render_widget(q, inner);

            if reveal {
                let ans_y = inner.y + 2;
                let ans_area = Rect {
                    x: inner.x,
                    y: ans_y,
                    width: inner.width,
                    height: inner.height.saturating_sub(2),
                };
                let mut text = vec![Line::from(vec![
                    Span::raw("A: ").style(title_style()),
                    Span::raw(&card.back),
                ])];
                if let Some(h) = &card.hint {
                    text.push(Line::from(vec![
                        Span::raw("hint: ").style(hint_style()),
                        Span::raw(h),
                    ]));
                }
                let a = Paragraph::new(text).wrap(Wrap { trim: true });
                f.render_widget(a, ans_area);
            }
        }
    }
}
