use crate::{Grade, Review};
use chrono::{Duration, NaiveDate};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Default)]
pub struct Totals {
    pub total: u32,
    pub hard: u32,
    pub medium: u32,
    pub easy: u32,
}

impl Totals {
    pub fn record(&mut self, g: &Grade) {
        self.total += 1;
        match g {
            Grade::Hard => self.hard += 1,
            Grade::Medium => self.medium += 1,
            Grade::Easy => self.easy += 1,
        }
    }
    pub fn accuracy(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.medium + self.easy) as f32 / self.total as f32
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct StatsSummary {
    pub totals: Totals,
    pub per_day: BTreeMap<NaiveDate, Totals>,
}

pub fn summarize(reviews: &[Review]) -> StatsSummary {
    let mut summary = StatsSummary::default();
    for r in reviews {
        summary.totals.record(&r.grade);
        let d = r.reviewed_at.date_naive();
        summary.per_day.entry(d).or_default().record(&r.grade);
    }
    summary
}

pub fn daily_streak(reviews: &[Review], today: NaiveDate) -> u32 {
    let per_day = summarize(reviews).per_day;
    let mut streak = 0u32;
    let mut day = today;
    loop {
        if per_day.get(&day).map(|t| t.total > 0).unwrap_or(false) {
            streak += 1;
            day -= Duration::days(1);
        } else {
            break;
        }
    }
    streak
}

pub fn per_deck_totals(
    reviews: &[Review],
    card_to_deck: &HashMap<uuid::Uuid, uuid::Uuid>,
) -> HashMap<uuid::Uuid, Totals> {
    let mut map: HashMap<uuid::Uuid, Totals> = HashMap::new();
    for r in reviews {
        if let Some(deck_id) = card_to_deck.get(&r.card_id) {
            map.entry(*deck_id).or_default().record(&r.grade);
        }
    }
    map
}
