use crate::{Card, DueStatus};
use chrono::{DateTime, Utc};

pub fn filter_by_text(cards: &[Card], query: &str) -> Vec<Card> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return cards.to_vec();
    }
    cards
        .iter()
        .filter(|c| {
            c.front.to_lowercase().contains(&q)
                || c.back.to_lowercase().contains(&q)
                || c.hint
                    .as_ref()
                    .map(|h| h.to_lowercase().contains(&q))
                    .unwrap_or(false)
                || c.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .cloned()
        .collect()
}

pub fn filter_by_tag(cards: &[Card], tag: &str) -> Vec<Card> {
    let q = tag.trim().to_lowercase();
    cards
        .iter()
        .filter(|c| c.tags.iter().any(|t| t.to_lowercase() == q))
        .cloned()
        .collect()
}

pub fn filter_by_due(cards: &[Card], now: DateTime<Utc>, want: DueStatus) -> Vec<Card> {
    cards
        .iter()
        .filter(|c| c.due_status(now) == want)
        .cloned()
        .collect()
}

pub fn filter_not_suspended(cards: &[Card]) -> Vec<Card> {
    cards.iter().filter(|c| !c.suspended).cloned().collect()
}
