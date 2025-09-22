use crate::{Card, Grade, Review, EF_MAX, EF_MIN};
use chrono::{Duration, Utc};

pub struct ScheduleOutcome {
    pub updated_card: Card,
    pub review: Review,
}

fn clamp_ef(x: f32) -> f32 {
    x.clamp(EF_MIN, EF_MAX)
}

pub fn apply_grade(mut card: Card, grade: Grade) -> ScheduleOutcome {
    let now = Utc::now();
    let g = grade.as_score();

    let new_ef = {
        let delta = 0.1 - (3 - g) as f32 * (0.08 + (3 - g) as f32 * 0.02);
        clamp_ef(card.ef + delta)
    };

    let new_reps;
    let new_interval;

    if g < 2 {
        new_reps = 0;
        new_interval = 1;
    } else {
        new_reps = card.reps + 1;
        new_interval = if new_reps == 1 {
            1
        } else if new_reps == 2 {
            6
        } else {
            let base = card.interval_days.max(1) as f32;
            (base * new_ef).round().max(1.0) as u32
        };
    }

    card.ef = new_ef;
    card.reps = new_reps;
    card.interval_days = new_interval;
    card.due_at = now + Duration::days(new_interval as i64);
    card.last_grade = Some(grade.clone());
    card.last_reviewed_at = Some(now);

    let review = Review::new(card.id, grade, now, new_interval as i32, new_ef);

    ScheduleOutcome { updated_card: card, review }
}
