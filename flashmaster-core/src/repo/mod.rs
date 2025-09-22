use crate::{Card, CardId, CoreError, Deck, DeckId, Review};
use async_trait::async_trait;

pub mod memory;

#[async_trait]
pub trait Repository: Send + Sync {
    // Decks
    async fn create_deck(&self, name: &str) -> Result<Deck, CoreError>;
    async fn get_deck(&self, id: DeckId) -> Result<Deck, CoreError>;
    async fn list_decks(&self) -> Result<Vec<Deck>, CoreError>;
    async fn delete_deck(&self, id: DeckId) -> Result<(), CoreError>;

    // Cards
    async fn add_card(
        &self,
        deck_id: DeckId,
        front: &str,
        back: &str,
        hint: Option<&str>,
        tags: &[String],
    ) -> Result<Card, CoreError>;

    async fn get_card(&self, id: CardId) -> Result<Card, CoreError>;
    async fn list_cards(&self, deck_id: Option<DeckId>) -> Result<Vec<Card>, CoreError>;
    async fn update_card(&self, card: &Card) -> Result<Card, CoreError>;
    async fn delete_card(&self, id: CardId) -> Result<(), CoreError>;
    async fn set_suspended(&self, id: CardId, suspended: bool) -> Result<(), CoreError>;

    // Reviews
    async fn insert_review(&self, review: &Review) -> Result<(), CoreError>;
    async fn list_reviews_for_card(&self, card_id: CardId) -> Result<Vec<Review>, CoreError>;
}
