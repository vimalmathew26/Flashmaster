use flashmaster_core::{
    daily_streak, filter_by_due, filter_by_tag, filter_by_text, summarize, Card, Deck, DueStatus,
    Grade, Review,
};
use chrono::{Duration, Utc};

#[test]
fn filters_text_and_tag() {
    let deck = Deck::new("Lang");
    let mut c1 = Card::new(deck.id, "hola", "hello");
    c1.tags = vec!["greeting".into(), "spanish".into()];
    let c2 = Card::new(deck.id, "adios", "goodbye");

    let v = vec![c1.clone(), c2.clone()];

    let by_text = filter_by_text(&v, "hol");
    assert_eq!(by_text.len(), 1);
    assert_eq!(by_text[0].front, "hola");

    let by_tag = filter_by_tag(&v, "spanish");
    assert_eq!(by_tag.len(), 1);
    assert_eq!(by_tag[0].front, "hola");
}

#[test]
fn filters_due() {
    let deck = Deck::new("Lang");
    let new_card = Card::new(deck.id, "hola", "hello");

    let mut due_card = Card::new(deck.id, "adios", "goodbye");
    let now = Utc::now();
    due_card.reps = 3;
    due_card.interval_days = 3;
    due_card.due_at = now;

    let mut future_card = Card::new(deck.id, "gracias", "thanks");
    future_card.reps = 1;
    future_card.interval_days = 2;
    future_card.due_at = now + Duration::days(2);

    let v = vec![new_card.clone(), due_card.clone(), future_card.clone()];

    let new_only = filter_by_due(&v, now, DueStatus::New);
    assert_eq!(new_only.len(), 1);

    let due_today = filter_by_due(&v, now, DueStatus::DueToday);
    assert_eq!(due_today.len(), 1);

    let future = filter_by_due(&v, now, DueStatus::Future);
    assert_eq!(future.len(), 1);
}

#[test]
fn stats_and_streak() {
    let deck = Deck::new("Lang");
    let card = Card::new(deck.id, "hola", "hello");
    let now = Utc::now();

    let r0 = Review::new(card.id, Grade::Easy, now - Duration::days(2), 1, 2.6);
    let r1 = Review::new(card.id, Grade::Medium, now - Duration::days(1), 6, 2.5);
    let r2 = Review::new(card.id, Grade::Hard, now, 1, 2.4);

    let s = summarize(&[r0.clone(), r1.clone(), r2.clone()]);
    assert_eq!(s.totals.total, 3);
    assert!(s.totals.accuracy() > 0.0);

    let today = now.date_naive();
    let streak = daily_streak(&[r0, r1, r2], today);
    assert!(streak >= 1);
}
