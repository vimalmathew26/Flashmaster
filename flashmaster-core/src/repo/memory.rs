use crate::{Card, CardId, CoreError, Deck, DeckId, Review};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;

#[derive(Default)]
pub struct MemoryRepo {
    decks: RwLock<HashMap<DeckId, Deck>>,
    cards: RwLock<HashMap<CardId, Card>>,
    reviews: RwLock<HashMap<CardId, Vec<Review>>>,
}

impl MemoryRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl crate::repo::Repository for MemoryRepo {
    async fn create_deck(&self, name: &str) -> Result<Deck, CoreError> {
        let deck = Deck::new(name);
        let mut m = self.decks.write();
        if m.values().any(|d| d.name.eq_ignore_ascii_case(name)) {
            return Err(CoreError::Conflict("deck name already exists"));
        }
        m.insert(deck.id, deck.clone());
        Ok(deck)
    }

    async fn get_deck(&self, id: DeckId) -> Result<Deck, CoreError> {
        self.decks
            .read()
            .get(&id)
            .cloned()
            .ok_or(CoreError::NotFound("deck"))
    }

    async fn list_decks(&self) -> Result<Vec<Deck>, CoreError> {
        Ok(self.decks.read().values().cloned().collect())
    }

    async fn delete_deck(&self, id: DeckId) -> Result<(), CoreError> {
        self.decks
            .write()
            .remove(&id)
            .ok_or(CoreError::NotFound("deck"))?;
        let mut cards = self.cards.write();
        let ids: Vec<CardId> = cards
            .values()
            .filter(|c| c.deck_id == id)
            .map(|c| c.id)
            .collect();
        for cid in ids {
            cards.remove(&cid);
            self.reviews.write().remove(&cid);
        }
        Ok(())
    }

    async fn add_card(
        &self,
        deck_id: DeckId,
        front: &str,
        back: &str,
        hint: Option<&str>,
        tags: &[String],
    ) -> Result<Card, CoreError> {
        if !self.decks.read().contains_key(&deck_id) {
            return Err(CoreError::NotFound("deck"));
        }
        let mut card = Card::new(deck_id, front, back);
        card.hint = hint.map(|s| s.to_string());
        card.tags = tags.to_vec();
        self.cards.write().insert(card.id, card.clone());
        Ok(card)
    }

    async fn get_card(&self, id: CardId) -> Result<Card, CoreError> {
        self.cards
            .read()
            .get(&id)
            .cloned()
            .ok_or(CoreError::NotFound("card"))
    }

    async fn list_cards(&self, deck_id: Option<DeckId>) -> Result<Vec<Card>, CoreError> {
        let cards = self.cards.read();
        let mut v: Vec<Card> = cards.values().cloned().collect();
        if let Some(did) = deck_id {
            v.retain(|c| c.deck_id == did);
        }
        Ok(v)
    }

    async fn update_card(&self, card: &Card) -> Result<Card, CoreError> {
        let mut m = self.cards.write();
        if !m.contains_key(&card.id) {
            return Err(CoreError::NotFound("card"));
        }
        m.insert(card.id, card.clone());
        Ok(card.clone())
    }

    async fn delete_card(&self, id: CardId) -> Result<(), CoreError> {
        self.cards
            .write()
            .remove(&id)
            .ok_or(CoreError::NotFound("card"))?;
        self.reviews.write().remove(&id);
        Ok(())
    }

    async fn set_suspended(&self, id: CardId, suspended: bool) -> Result<(), CoreError> {
        let mut m = self.cards.write();
        let Some(card) = m.get_mut(&id) else {
            return Err(CoreError::NotFound("card"));
        };
        card.suspended = suspended;
        Ok(())
    }

    async fn insert_review(&self, review: &Review) -> Result<(), CoreError> {
        let mut m = self.reviews.write();
        m.entry(review.card_id).or_default().push(review.clone());
        Ok(())
    }

    async fn list_reviews_for_card(&self, card_id: CardId) -> Result<Vec<Review>, CoreError> {
        Ok(self
            .reviews
            .read()
            .get(&card_id)
            .cloned()
            .unwrap_or_default())
    }
}
