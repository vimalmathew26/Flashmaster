use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Quit,
    Up,
    Down,
    Enter,
    ToggleReveal,
    GradeHard,
    GradeMedium,
    GradeEasy,
    Skip,
    None,
}

pub fn map_event(ev: Event) -> Action {
    if let Event::Key(KeyEvent {
        code, modifiers, ..
    }) = ev
    {
        match (code, modifiers) {
            (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => Action::Quit,
            (KeyCode::Up, _) | (KeyCode::Char('k'), _) => Action::Up,
            (KeyCode::Down, _) | (KeyCode::Char('j'), _) => Action::Down,
            (KeyCode::Enter, _) => Action::Enter,
            (KeyCode::Char(' '), _) => Action::ToggleReveal,
            (KeyCode::Char('1'), _) | (KeyCode::Char('h'), _) => Action::GradeHard,
            (KeyCode::Char('2'), _) | (KeyCode::Char('m'), _) => Action::GradeMedium,
            (KeyCode::Char('3'), _) | (KeyCode::Char('e'), _) => Action::GradeEasy,
            (KeyCode::Char('s'), KeyModifiers::NONE) => Action::Skip,
            _ => Action::None,
        }
    } else {
        Action::None
    }
}
