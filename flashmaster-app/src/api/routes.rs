use axum::{extract::{Query, State}, http::StatusCode, Json};
use serde::Deserialize;
use std::sync::Arc;

use flashmaster_core::{
    filters::{filter_by_due, filter_not_suspended},
    scheduler::apply_grade,
    DueStatus,
};

use crate::api::dto::{CardOut, DeckOut, ReviewIn, parse_grade};

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<dyn flashmaster_core::Repository>,
}

#[derive(Deserialize)]
pub struct DueQuery {
    deck: Option<String>,
    include_new: Option<bool>,
    include_lapsed: Option<bool>,
    max: Option<usize>,
}

pub async fn list_decks(State(st): State<Arc<AppState>>) -> Result<Json<Vec<DeckOut>>, StatusCode> {
    let mut decks = st.repo.list_decks().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    decks.sort_by_key(|d| d.created_at);
    Ok(Json(decks.into_iter().map(|d| DeckOut { id: d.id, name: d.name, created_at: d.created_at }).collect()))
}

pub async fn due_cards(State(st): State<Arc<AppState>>, Query(q): Query<DueQuery>)
    -> Result<Json<Vec<CardOut>>, StatusCode>
{
    let now = chrono::Utc::now();
    let deck_id = if let Some(sel) = q.deck.clone() {
        Some(super::server::resolve_deck(&*st.repo, &sel).await.map_err(|_| StatusCode::BAD_REQUEST)?.id)
    } else { None };

    let mut cards = st.repo.list_cards(deck_id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    cards = filter_not_suspended(&cards);

    let mut pool = Vec::new();
    if q.include_new.unwrap_or(false) { pool.extend(filter_by_due(&cards, now, DueStatus::New)); }
    pool.extend(filter_by_due(&cards, now, DueStatus::DueToday));
    if q.include_lapsed.unwrap_or(false) { pool.extend(filter_by_due(&cards, now, DueStatus::Lapsed)); }
    pool.sort_by_key(|c| (c.due_at, c.created_at));
    if let Some(m) = q.max { pool.truncate(m); }

    Ok(Json(pool.into_iter().map(|c| CardOut {
        id: c.id, deck_id: c.deck_id, front: c.front, back: c.back, hint: c.hint, tags: c.tags,
        due_at: c.due_at, suspended: c.suspended
    }).collect()))
}

pub async fn post_review(State(st): State<Arc<AppState>>, Json(body): Json<ReviewIn>) -> Result<StatusCode, StatusCode> {
    let card = st.repo.get_card(body.card_id).await.map_err(|_| StatusCode::BAD_REQUEST)?;
    let grade = parse_grade(&body.grade).ok_or(StatusCode::BAD_REQUEST)?;
    let out = apply_grade(card, grade);
    st.repo.update_card(&out.updated_card).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    st.repo.insert_review(&out.review).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}
