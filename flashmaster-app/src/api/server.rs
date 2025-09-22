use axum::{routing::{get, post}, Router};
use std::{net::SocketAddr, sync::Arc};
use tower_http::trace::TraceLayer;
use tokio::net::TcpListener;

use flashmaster_core::{Repository, Deck};
use crate::api::routes::{AppState, list_decks, due_cards, post_review};

pub async fn run(repo: Arc<dyn Repository>, addr: SocketAddr) -> anyhow::Result<()> {
    let state = Arc::new(AppState { repo });

    let app = Router::new()
        .route("/decks", get(list_decks))
        .route("/due", get(due_cards))
        .route("/review", post(post_review))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

pub async fn resolve_deck<R: Repository + ?Sized>(repo: &R, sel: &str) -> anyhow::Result<Deck> {
    if let Ok(id) = uuid::Uuid::parse_str(sel) {
        if let Ok(d) = repo.get_deck(id).await { return Ok(d); }
    }
    let decks = repo.list_decks().await?;
    if let Some(d) = decks.into_iter().find(|d| d.name.eq_ignore_ascii_case(sel)) {
        return Ok(d);
    }
    anyhow::bail!("deck not found")
}
