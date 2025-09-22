use chrono::{DateTime, Utc};
use flashmaster_core::{repo::Repository, Card, CardId, CoreError, Deck, DeckId, Review};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use tokio::task;

pub mod paths;

const FILE_VERSION: u32 = 1;

#[derive(Clone, Serialize, Deserialize)]
struct FileImage {
    version: u32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    decks: Vec<Deck>,
    cards: Vec<Card>,
    reviews: Vec<Review>,
}

#[derive(Default, Clone)]
struct State {
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    decks: HashMap<DeckId, Deck>,
    cards: HashMap<CardId, Card>,
    reviews: HashMap<CardId, Vec<Review>>,
}

impl State {
    fn new_empty() -> Self {
        let now = Utc::now();
        Self {
            created_at: now,
            updated_at: now,
            decks: HashMap::new(),
            cards: HashMap::new(),
            reviews: HashMap::new(),
        }
    }

    fn to_image(&self) -> FileImage {
        FileImage {
            version: FILE_VERSION,
            created_at: self.created_at,
            updated_at: self.updated_at,
            decks: self.decks.values().cloned().collect(),
            cards: self.cards.values().cloned().collect(),
            reviews: self
                .reviews
                .values()
                .flat_map(|v| v.clone().into_iter())
                .collect(),
        }
    }

    fn from_image(img: FileImage) -> Self {
        let mut decks = HashMap::new();
        for d in img.decks {
            decks.insert(d.id, d);
        }
        let mut cards = HashMap::new();
        for c in img.cards {
            cards.insert(c.id, c);
        }
        let mut reviews: HashMap<CardId, Vec<Review>> = HashMap::new();
        for r in img.reviews {
            reviews.entry(r.card_id).or_default().push(r);
        }
        Self {
            created_at: img.created_at,
            updated_at: img.updated_at,
            decks,
            cards,
            reviews,
        }
    }
}

pub struct JsonStore {
    path: PathBuf,
    backups_dir: PathBuf,
    max_backups: usize,
    state: RwLock<State>,
}

impl JsonStore {
    pub async fn open_default() -> Result<Self, CoreError> {
        let (file, backups) = paths::default_store_file();
        Self::open_with(file, backups, 10).await
    }

    pub async fn open_with(path: PathBuf, backups_dir: PathBuf, max_backups: usize) -> Result<Self, CoreError> {
        ensure_parent_dirs(&path)?;
        ensure_dir(&backups_dir)?;
        let state = load_or_init(&path).await?;
        Ok(Self {
            path,
            backups_dir,
            max_backups: max_backups.max(1),
            state: RwLock::new(state),
        })
    }

    async fn save(&self) -> Result<(), CoreError> {
        let snapshot = {
            let mut s = self.state.write();
            s.updated_at = Utc::now();
            s.to_image()
        };
        let path = self.path.clone();
        let backups = self.backups_dir.clone();
        let keep = self.max_backups;

        // Join error -> CoreError, inner io::Error -> CoreError
        task::spawn_blocking(move || write_with_backup(&path, &backups, keep, &snapshot))
            .await
            .map_err(|_| CoreError::Storage("io"))?
            .map_err(|_| CoreError::Storage("io"))?;
        Ok(())
    }
}

fn ensure_parent_dirs(path: &Path) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    Ok(())
}

fn ensure_dir(path: &Path) -> Result<(), CoreError> {
    fs::create_dir_all(path).map_err(|_| CoreError::Storage("io"))
}

async fn load_or_init(path: &Path) -> Result<State, CoreError> {
    if path.exists() {
        let p = path.to_path_buf();
        let img: FileImage = task::spawn_blocking(move || {
            let mut f = fs::File::open(&p)?;
            let mut buf = String::new();
            f.read_to_string(&mut buf)?;
            let v = serde_json::from_str::<FileImage>(&buf)?;
            Ok::<FileImage, std::io::Error>(v)
        })
        .await
        .map_err(|_| CoreError::Storage("io"))
        .and_then(|r| r.map_err(|_| CoreError::Storage("io")))?;
        let mut st = State::from_image(img);
        st.updated_at = Utc::now();
        Ok(st)
    } else {
        let st = State::new_empty();
        let img = st.to_image();
        write_with_backup(path, &path.with_extension("backups"), 1, &img).map_err(|_| CoreError::Storage("io"))?;
        Ok(st)
    }
}

fn write_with_backup(path: &Path, backups_dir: &Path, max_backups: usize, img: &FileImage) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(backups_dir)?;

    let json = serde_json::to_vec_pretty(img).expect("serialize");
    let mut tmp = NamedTempFile::new_in(path.parent().unwrap_or_else(|| Path::new(".")))?;
    tmp.write_all(&json)?;
    tmp.flush()?;
    let _ = fs::remove_file(path);
    tmp.persist(path)?;

    // Backup rotation
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let backup_name = format!("flashmaster-{ts}.json");
    let backup_path = backups_dir.join(backup_name);
    let mut btmp = NamedTempFile::new_in(backups_dir)?;
    btmp.write_all(&json)?;
    btmp.flush()?;
    let _ = fs::remove_file(&backup_path);
    btmp.persist(&backup_path)?;

    rotate_backups(backups_dir, max_backups)?;

    Ok(())
}

fn rotate_backups(dir: &Path, keep: usize) -> Result<(), std::io::Error> {
    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    if entries.len() > keep {
        for e in &entries[0..entries.len() - keep] {
            let _ = fs::remove_file(e.path());
        }
    }
    Ok(())
}

use async_trait::async_trait;

#[async_trait]
impl Repository for JsonStore {
    async fn create_deck(&self, name: &str) -> Result<Deck, CoreError> {
        let deck = Deck::new(name);
        {
            let mut s = self.state.write();
            if s.decks.values().any(|d| d.name.eq_ignore_ascii_case(name)) {
                return Err(CoreError::Conflict("deck name already exists"));
            }
            s.decks.insert(deck.id, deck.clone());
        }
        self.save().await?;
        Ok(deck)
    }

    async fn get_deck(&self, id: DeckId) -> Result<Deck, CoreError> {
        let s = self.state.read();
        s.decks.get(&id).cloned().ok_or(CoreError::NotFound("deck"))
    }

    async fn list_decks(&self) -> Result<Vec<Deck>, CoreError> {
        let s = self.state.read();
        Ok(s.decks.values().cloned().collect())
    }

    async fn delete_deck(&self, id: DeckId) -> Result<(), CoreError> {
        {
            let mut s = self.state.write();
            if s.decks.remove(&id).is_none() {
                return Err(CoreError::NotFound("deck"));
            }
            let to_remove: Vec<CardId> = s.cards.values().filter(|c| c.deck_id == id).map(|c| c.id).collect();
            for cid in to_remove {
                s.cards.remove(&cid);
                s.reviews.remove(&cid);
            }
        }
        self.save().await
    }

    async fn add_card(
        &self,
        deck_id: DeckId,
        front: &str,
        back: &str,
        hint: Option<&str>,
        tags: &[String],
    ) -> Result<Card, CoreError> {
        let card = {
            let s = self.state.read();
            if !s.decks.contains_key(&deck_id) {
                return Err(CoreError::NotFound("deck"));
            }
            let mut c = Card::new(deck_id, front, back);
            c.hint = hint.map(|s| s.to_string());
            c.tags = tags.to_vec();
            c
        };
        {
            let mut s = self.state.write();
            s.cards.insert(card.id, card.clone());
        }
        self.save().await?;
        Ok(card)
    }

    async fn get_card(&self, id: CardId) -> Result<Card, CoreError> {
        let s = self.state.read();
        s.cards.get(&id).cloned().ok_or(CoreError::NotFound("card"))
    }

    async fn list_cards(&self, deck_id: Option<DeckId>) -> Result<Vec<Card>, CoreError> {
        let s = self.state.read();
        let mut v: Vec<Card> = s.cards.values().cloned().collect();
        if let Some(did) = deck_id {
            v.retain(|c| c.deck_id == did);
        }
        Ok(v)
    }

    async fn update_card(&self, card: &Card) -> Result<Card, CoreError> {
        {
            let mut s = self.state.write();
            if !s.cards.contains_key(&card.id) {
                return Err(CoreError::NotFound("card"));
            }
            s.cards.insert(card.id, card.clone());
        }
        self.save().await?;
        Ok(card.clone())
    }

    async fn delete_card(&self, id: CardId) -> Result<(), CoreError> {
        {
            let mut s = self.state.write();
            if s.cards.remove(&id).is_none() {
                return Err(CoreError::NotFound("card"));
            }
            s.reviews.remove(&id);
        }
        self.save().await
    }

    async fn set_suspended(&self, id: CardId, suspended: bool) -> Result<(), CoreError> {
        {
            let mut s = self.state.write();
            let Some(c) = s.cards.get_mut(&id) else {
                return Err(CoreError::NotFound("card"));
            };
            c.suspended = suspended;
        }
        self.save().await
    }

    async fn insert_review(&self, review: &Review) -> Result<(), CoreError> {
        {
            let mut s = self.state.write();
            s.reviews.entry(review.card_id).or_default().push(review.clone());
        }
        self.save().await
    }

    async fn list_reviews_for_card(&self, card_id: CardId) -> Result<Vec<Review>, CoreError> {
        let s = self.state.read();
        Ok(s.reviews.get(&card_id).cloned().unwrap_or_default())
    }
}
