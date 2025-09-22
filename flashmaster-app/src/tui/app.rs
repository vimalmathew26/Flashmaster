use crate::tui::{inputs::{map_event, Action}, views::{self, RightPane}};
use crossterm::{
    event::{self},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use flashmaster_core::{
    filters::{filter_by_due, filter_not_suspended},
    scheduler::apply_grade,
    Card, Deck, DueStatus, Grade, Repository,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{stdout, Stdout};
use std::sync::Arc;
use tokio::runtime::Runtime;

pub struct TuiApp {
    pub repo: Arc<dyn Repository>,
    pub rt: Arc<Runtime>,
    decks: Vec<Deck>,
    sel: usize,
    queue: Vec<Card>,
    idx: usize,
    reveal: bool,
    in_review: bool,
}

impl TuiApp {
    pub fn new(repo: Arc<dyn Repository>, rt: Arc<Runtime>) -> Self {
        Self { repo, rt, decks: vec![], sel: 0, queue: vec![], idx: 0, reveal: false, in_review: false }
    }

    fn load_decks(&mut self) {
        let mut v = self.rt.block_on(self.repo.list_decks()).unwrap_or_default();
        v.sort_by_key(|d| d.created_at);
        self.decks = v;
        self.sel = self.sel.min(self.decks.len().saturating_sub(1));
    }

    fn build_queue(&mut self) {
        self.queue.clear();
        self.idx = 0;
        self.reveal = false;
        if self.decks.is_empty() { return; }
        let did = self.decks[self.sel].id;
        let mut cards = self.rt.block_on(self.repo.list_cards(Some(did))).unwrap_or_default();
        cards = filter_not_suspended(&cards);
        let now = chrono::Utc::now();
        let mut pool = Vec::new();
        pool.extend(filter_by_due(&cards, now, DueStatus::DueToday));
        pool.extend(filter_by_due(&cards, now, DueStatus::New));
        pool.extend(filter_by_due(&cards, now, DueStatus::Lapsed));
        pool.sort_by_key(|c| (c.due_at, c.created_at));
        self.queue = pool;
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        self.load_decks();

        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let res = self.mainloop(&mut terminal);

        disable_raw_mode().ok();
        let mut out: Stdout = std::io::stdout();
        execute!(out, LeaveAlternateScreen).ok();
        terminal.show_cursor().ok();

        res
    }

    fn mainloop(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
        loop {
            terminal.draw(|f| {
                let right = if self.in_review {
                    if let Some(card) = self.queue.get(self.idx) { RightPane::Card { card, reveal: self.reveal } }
                    else { RightPane::Empty("No cards in queue.") }
                } else { RightPane::Idle };
                views::draw_ui(f, f.size(), &self.decks, self.sel, right);
            })?;

            if event::poll(std::time::Duration::from_millis(100))? {
                let ev = event::read()?;
                let action = map_event(ev);
                match action {
                    Action::Quit => break,
                    Action::Up   => { if !self.in_review { self.sel = self.sel.saturating_sub(1); } }
                    Action::Down => { if !self.in_review && self.sel + 1 < self.decks.len() { self.sel += 1; } }
                    Action::Enter => {
                        if !self.in_review {
                            self.build_queue();
                            self.in_review = true;
                            self.idx = 0;
                            self.reveal = false;
                        }
                    }
                    Action::ToggleReveal => { if self.in_review { self.reveal = !self.reveal; } }
                    Action::Skip => {
                        if self.in_review && self.idx + 1 < self.queue.len() { self.idx += 1; self.reveal = false; }
                    }
                    Action::GradeHard | Action::GradeMedium | Action::GradeEasy => {
                        if self.in_review {
                            if let Some(card) = self.queue.get(self.idx).cloned() {
                                let grade = match action {
                                    Action::GradeHard => Grade::Hard,
                                    Action::GradeMedium => Grade::Medium,
                                    Action::GradeEasy => Grade::Easy,
                                    _ => Grade::Medium,
                                };
                                let out = apply_grade(card, grade);
                                self.rt.block_on(self.repo.update_card(&out.updated_card)).ok();
                                self.rt.block_on(self.repo.insert_review(&out.review)).ok();
                                if self.idx + 1 < self.queue.len() { self.idx += 1; self.reveal = false; } else { self.in_review = false; }
                            }
                        }
                    }
                    Action::None => {}
                }
            }
        }
        Ok(())
    }
}
