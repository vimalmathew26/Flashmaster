use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type DeckId = Uuid;
pub type CardId = Uuid;
pub type ReviewId = Uuid;

pub const EF_MIN: f32 = 1.3;
pub const EF_MAX: f32 = 2.8;
pub const EF_DEFAULT: f32 = 2.5;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    Hard,
    Medium,
    Easy,
}

impl Grade {
    pub fn as_score(&self) -> i32 {
        match self {
            Grade::Hard => 1,
            Grade::Medium => 2,
            Grade::Easy => 3,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DueStatus {
    New,
    DueToday,
    Lapsed,
    Future,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Deck {
    pub id: DeckId,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

impl Deck {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            created_at: Utc::now(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Card {
    pub id: CardId,
    pub deck_id: DeckId,
    pub front: String,
    pub back: String,
    pub hint: Option<String>,
    pub tags: Vec<String>,

    pub reps: u32,
    pub interval_days: u32,
    pub ef: f32,
    pub due_at: DateTime<Utc>,
    pub last_grade: Option<Grade>,
    pub last_reviewed_at: Option<DateTime<Utc>>,
    pub suspended: bool,

    pub created_at: DateTime<Utc>,
}

impl Card {
    pub fn new(deck_id: DeckId, front: impl Into<String>, back: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            deck_id,
            front: front.into(),
            back: back.into(),
            hint: None,
            tags: Vec::new(),
            reps: 0,
            interval_days: 0,
            ef: EF_DEFAULT,
            due_at: Utc::now(),
            last_grade: None,
            last_reviewed_at: None,
            suspended: false,
            created_at: Utc::now(),
        }
    }

    pub fn is_new(&self) -> bool {
        self.reps == 0
    }

    pub fn due_status(&self, now: DateTime<Utc>) -> crate::DueStatus {
        if self.is_new() {
            crate::DueStatus::New
        } else if self.due_at > now {
            crate::DueStatus::Future
        } else {
            let elapsed = now - self.due_at;
            if elapsed.num_hours() >= 24 {
                crate::DueStatus::Lapsed
            } else {
                crate::DueStatus::DueToday
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Review {
    pub id: ReviewId,
    pub card_id: CardId,
    pub grade: Grade,
    pub reviewed_at: DateTime<Utc>,
    pub interval_applied: i32,
    pub ef_after: f32,
}

impl Review {
    pub fn new(
        card_id: CardId,
        grade: Grade,
        reviewed_at: DateTime<Utc>,
        interval_applied: i32,
        ef_after: f32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            card_id,
            grade,
            reviewed_at,
            interval_applied,
            ef_after,
        }
    }
}
