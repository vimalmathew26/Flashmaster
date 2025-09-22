use flashmaster_core::{apply_grade, Card, Deck, Grade, EF_MAX, EF_MIN};
use chrono::{Duration, Utc};

#[test]
fn easy_from_new() {
    let deck = Deck::new("Test");
    let card = Card::new(deck.id, "hola", "hello");
    let before = Utc::now();

    let out = apply_grade(card, Grade::Easy);
    let c = out.updated_card;

    assert_eq!(c.reps, 1);
    assert_eq!(c.interval_days, 1);
    assert!(c.ef > 2.5 && c.ef <= EF_MAX);
    assert!(c.due_at >= before + Duration::days(1));
    assert_eq!(c.last_grade, Some(Grade::Easy));
    assert!(out.review.interval_applied >= 1);
}

#[test]
fn medium_progression() {
    let deck = Deck::new("Test");
    let mut card = Card::new(deck.id, "a", "b");
    // first correct to bump reps to 1
    let out1 = apply_grade(card, Grade::Medium);
    card = out1.updated_card;
    assert_eq!(card.reps, 1);
    assert_eq!(card.interval_days, 1);

    // second correct should set interval to 6
    let out2 = apply_grade(card, Grade::Medium);
    let c2 = out2.updated_card;
    assert_eq!(c2.reps, 2);
    assert_eq!(c2.interval_days, 6);
}

#[test]
fn hard_resets_interval() {
    let deck = Deck::new("Test");
    let mut card = Card::new(deck.id, "x", "y");
    let out1 = apply_grade(card, Grade::Easy);
    card = out1.updated_card;

    let out2 = apply_grade(card, Grade::Hard);
    let c2 = out2.updated_card;

    assert_eq!(c2.reps, 0);
    assert_eq!(c2.interval_days, 1);
    assert!(c2.ef >= EF_MIN && c2.ef <= EF_MAX);
    assert_eq!(c2.last_grade, Some(Grade::Hard));
}
