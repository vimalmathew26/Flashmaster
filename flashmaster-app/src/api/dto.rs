use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize)]
pub struct DeckOut {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct CardOut {
    pub id: Uuid,
    pub deck_id: Uuid,
    pub front: String,
    pub back: String,
    pub hint: Option<String>,
    pub tags: Vec<String>,
    pub due_at: DateTime<Utc>,
    pub suspended: bool,
}

#[derive(Deserialize)]
pub struct ReviewIn {
    pub card_id: Uuid,
    pub grade: String,
}

pub fn parse_grade(s: &str) -> Option<flashmaster_core::Grade> {
    match s.to_lowercase().as_str() {
        "1" | "h" | "hard" => Some(flashmaster_core::Grade::Hard),
        "2" | "m" | "med" | "medium" => Some(flashmaster_core::Grade::Medium),
        "3" | "e" | "easy" => Some(flashmaster_core::Grade::Easy),
        _ => None,
    }
}
