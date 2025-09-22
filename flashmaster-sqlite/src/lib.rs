use chrono::{DateTime, Utc};
use flashmaster_core::{repo::Repository, Card, CardId, CoreError, Deck, DeckId, Grade, Review};
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};
use std::path::Path;

pub struct SqliteRepo {
    pool: SqlitePool,
}

impl SqliteRepo {
    pub async fn open_file(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let url = format!("sqlite://{}", path.as_ref().to_string_lossy());
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .map_err(|_| CoreError::Storage("sqlite connect"))?;
        let repo = Self { pool };
        repo.ensure_schema().await?;
        Ok(repo)
    }

    pub async fn open_memory() -> Result<Self, CoreError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect("sqlite::memory:")
            .await
            .map_err(|_| CoreError::Storage("sqlite connect"))?;
        let repo = Self { pool };
        repo.ensure_schema().await?;
        Ok(repo)
    }

    async fn ensure_schema(&self) -> Result<(), CoreError> {
        // Create tables/indexes if they do not exist (mirrors migrations).
        const STMT: &str = r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS decks (
          id          TEXT PRIMARY KEY,
          name        TEXT NOT NULL UNIQUE,
          created_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS cards (
          id                TEXT PRIMARY KEY,
          deck_id           TEXT NOT NULL,
          front             TEXT NOT NULL,
          back              TEXT NOT NULL,
          hint              TEXT,
          tags              TEXT NOT NULL,
          reps              INTEGER NOT NULL DEFAULT 0,
          interval_days     INTEGER NOT NULL DEFAULT 0,
          ef                REAL    NOT NULL DEFAULT 2.5,
          due_at            TEXT    NOT NULL,
          last_grade        INTEGER,
          last_reviewed_at  TEXT,
          suspended         INTEGER NOT NULL DEFAULT 0,
          created_at        TEXT NOT NULL,
          FOREIGN KEY(deck_id) REFERENCES decks(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS reviews (
          id               TEXT PRIMARY KEY,
          card_id          TEXT NOT NULL,
          grade            INTEGER NOT NULL,
          reviewed_at      TEXT NOT NULL,
          interval_applied INTEGER NOT NULL,
          ef_after         REAL NOT NULL,
          FOREIGN KEY(card_id) REFERENCES cards(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_cards_deck_due ON cards (deck_id, due_at);
        CREATE INDEX IF NOT EXISTS idx_reviews_card_time ON reviews (card_id, reviewed_at);
        "#;

        // Execute statements one by one for compatibility.
        for chunk in STMT.split(';') {
            let sql = chunk.trim();
            if sql.is_empty() {
                continue;
            }
            sqlx::query(sql)
                .execute(&self.pool)
                .await
                .map_err(|_| CoreError::Storage("sqlite schema"))?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl Repository for SqliteRepo {
    // ===== Decks =====
    async fn create_deck(&self, name: &str) -> Result<Deck, CoreError> {
        // Pre-check for unique name
        let exists: Option<i64> =
            sqlx::query("SELECT 1 FROM decks WHERE lower(name)=lower(?) LIMIT 1")
                .bind(name)
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| CoreError::Storage("read deck"))?
                .map(|_| 1);
        if exists.is_some() {
            return Err(CoreError::Conflict("deck name already exists"));
        }

        let deck = Deck::new(name);
        sqlx::query("INSERT INTO decks (id,name,created_at) VALUES (?,?,?)")
            .bind(deck.id.to_string())
            .bind(&deck.name)
            .bind(dt_to_str(deck.created_at))
            .execute(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("insert deck"))?;
        Ok(deck)
    }

    async fn get_deck(&self, id: DeckId) -> Result<Deck, CoreError> {
        let row = sqlx::query("SELECT id,name,created_at FROM decks WHERE id=?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("read deck"))?;
        let row = row.ok_or(CoreError::NotFound("deck"))?;
        Ok(Deck {
            id: uuid_from_str(row.get::<String, _>("id"))?,
            name: row.get::<String, _>("name"),
            created_at: dt_from_str(row.get::<String, _>("created_at"))?,
        })
    }

    async fn list_decks(&self) -> Result<Vec<Deck>, CoreError> {
        let rows = sqlx::query("SELECT id,name,created_at FROM decks ORDER BY created_at ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("list decks"))?;
        let mut v = Vec::with_capacity(rows.len());
        for row in rows {
            v.push(Deck {
                id: uuid_from_str(row.get::<String, _>("id"))?,
                name: row.get::<String, _>("name"),
                created_at: dt_from_str(row.get::<String, _>("created_at"))?,
            });
        }
        Ok(v)
    }

    async fn delete_deck(&self, id: DeckId) -> Result<(), CoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| CoreError::Storage("tx"))?;

        // Manual cascade (robust even if PRAGMA foreign_keys is off)
        sqlx::query("DELETE FROM reviews WHERE card_id IN (SELECT id FROM cards WHERE deck_id=?)")
            .bind(id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| CoreError::Storage("del reviews"))?;

        sqlx::query("DELETE FROM cards WHERE deck_id=?")
            .bind(id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| CoreError::Storage("del cards"))?;

        let res = sqlx::query("DELETE FROM decks WHERE id=?")
            .bind(id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| CoreError::Storage("del deck"))?;
        if res.rows_affected() == 0 {
            tx.rollback().await.ok();
            return Err(CoreError::NotFound("deck"));
        }

        tx.commit()
            .await
            .map_err(|_| CoreError::Storage("tx commit"))
    }

    // ===== Cards =====
    async fn add_card(
        &self,
        deck_id: DeckId,
        front: &str,
        back: &str,
        hint: Option<&str>,
        tags: &[String],
    ) -> Result<Card, CoreError> {
        // Ensure deck exists
        let exists = sqlx::query("SELECT 1 FROM decks WHERE id=? LIMIT 1")
            .bind(deck_id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("read deck"))?
            .is_some();
        if !exists {
            return Err(CoreError::NotFound("deck"));
        }

        let mut card = Card::new(deck_id, front, back);
        card.hint = hint.map(|s| s.to_string());
        card.tags = tags.to_vec();

        sqlx::query(
            r#"
            INSERT INTO cards (
              id, deck_id, front, back, hint, tags, reps, interval_days, ef, due_at,
              last_grade, last_reviewed_at, suspended, created_at
            )
            VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)
            "#,
        )
        .bind(card.id.to_string())
        .bind(card.deck_id.to_string())
        .bind(&card.front)
        .bind(&card.back)
        .bind(card.hint.clone())
        .bind(serde_json::to_string(&card.tags).unwrap())
        .bind(card.reps as i64)
        .bind(card.interval_days as i64)
        .bind(card.ef as f64)
        .bind(dt_to_str(card.due_at))
        .bind(card.last_grade.as_ref().map(grade_to_i))
        .bind(card.last_reviewed_at.map(dt_to_str))
        .bind(bool_to_i(card.suspended))
        .bind(dt_to_str(card.created_at))
        .execute(&self.pool)
        .await
        .map_err(|_| CoreError::Storage("insert card"))?;

        Ok(card)
    }

    async fn get_card(&self, id: CardId) -> Result<Card, CoreError> {
        let row = sqlx::query(
            r#"SELECT id,deck_id,front,back,hint,tags,reps,interval_days,ef,due_at,
                       last_grade,last_reviewed_at,suspended,created_at
               FROM cards WHERE id=?"#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CoreError::Storage("read card"))?;
        let row = row.ok_or(CoreError::NotFound("card"))?;
        Ok(row_into_card(row)?)
    }

    async fn list_cards(&self, deck_id: Option<DeckId>) -> Result<Vec<Card>, CoreError> {
        let rows = if let Some(did) = deck_id {
            sqlx::query(
                r#"SELECT id,deck_id,front,back,hint,tags,reps,interval_days,ef,due_at,
                          last_grade,last_reviewed_at,suspended,created_at
                   FROM cards WHERE deck_id=? ORDER BY created_at ASC"#,
            )
            .bind(did.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("list cards"))?
        } else {
            sqlx::query(
                r#"SELECT id,deck_id,front,back,hint,tags,reps,interval_days,ef,due_at,
                          last_grade,last_reviewed_at,suspended,created_at
                   FROM cards ORDER BY created_at ASC"#,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("list cards"))?
        };
        let mut v = Vec::with_capacity(rows.len());
        for row in rows {
            v.push(row_into_card(row)?);
        }
        Ok(v)
    }

    async fn update_card(&self, card: &Card) -> Result<Card, CoreError> {
        let res = sqlx::query(
            r#"
            UPDATE cards SET
              deck_id=?, front=?, back=?, hint=?, tags=?, reps=?, interval_days=?,
              ef=?, due_at=?, last_grade=?, last_reviewed_at=?, suspended=?
            WHERE id=?
            "#,
        )
        .bind(card.deck_id.to_string())
        .bind(&card.front)
        .bind(&card.back)
        .bind(card.hint.clone())
        .bind(serde_json::to_string(&card.tags).unwrap())
        .bind(card.reps as i64)
        .bind(card.interval_days as i64)
        .bind(card.ef as f64)
        .bind(dt_to_str(card.due_at))
        .bind(card.last_grade.as_ref().map(grade_to_i))
        .bind(card.last_reviewed_at.map(dt_to_str))
        .bind(bool_to_i(card.suspended))
        .bind(card.id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|_| CoreError::Storage("update card"))?;
        if res.rows_affected() == 0 {
            return Err(CoreError::NotFound("card"));
        }
        Ok(card.clone())
    }

    async fn delete_card(&self, id: CardId) -> Result<(), CoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| CoreError::Storage("tx"))?;
        sqlx::query("DELETE FROM reviews WHERE card_id=?")
            .bind(id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| CoreError::Storage("del reviews"))?;
        let res = sqlx::query("DELETE FROM cards WHERE id=?")
            .bind(id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| CoreError::Storage("del card"))?;
        if res.rows_affected() == 0 {
            tx.rollback().await.ok();
            return Err(CoreError::NotFound("card"));
        }
        tx.commit()
            .await
            .map_err(|_| CoreError::Storage("tx commit"))
    }

    async fn set_suspended(&self, id: CardId, suspended: bool) -> Result<(), CoreError> {
        let res = sqlx::query("UPDATE cards SET suspended=? WHERE id=?")
            .bind(bool_to_i(suspended))
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("suspend"))?;
        if res.rows_affected() == 0 {
            return Err(CoreError::NotFound("card"));
        }
        Ok(())
    }

    // ===== Reviews =====
    async fn insert_review(&self, review: &Review) -> Result<(), CoreError> {
        sqlx::query(
            r#"INSERT INTO reviews (id,card_id,grade,reviewed_at,interval_applied,ef_after)
               VALUES (?,?,?,?,?,?)"#,
        )
        .bind(review.id.to_string())
        .bind(review.card_id.to_string())
        .bind(grade_to_i(&review.grade))
        .bind(dt_to_str(review.reviewed_at))
        .bind(review.interval_applied as i64)
        .bind(review.ef_after as f64)
        .execute(&self.pool)
        .await
        .map_err(|_| CoreError::Storage("insert review"))?;
        Ok(())
    }

    async fn list_reviews_for_card(&self, card_id: CardId) -> Result<Vec<Review>, CoreError> {
        let rows = sqlx::query(
            r#"SELECT id,card_id,grade,reviewed_at,interval_applied,ef_after
               FROM reviews WHERE card_id=? ORDER BY reviewed_at ASC"#,
        )
        .bind(card_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CoreError::Storage("list reviews"))?;
        let mut v = Vec::with_capacity(rows.len());
        for row in rows {
            v.push(Review {
                id: uuid_from_str(row.get::<String, _>("id"))?,
                card_id: uuid_from_str(row.get::<String, _>("card_id"))?,
                grade: grade_from_i(row.get::<i64, _>("grade"))
                    .ok_or(CoreError::Invalid("grade"))?,
                reviewed_at: dt_from_str(row.get::<String, _>("reviewed_at"))?,
                interval_applied: row.get::<i64, _>("interval_applied") as i32,
                ef_after: row.get::<f64, _>("ef_after") as f32,
            });
        }
        Ok(v)
    }
}

// ===== Helpers =====
fn uuid_from_str(s: String) -> Result<uuid::Uuid, CoreError> {
    uuid::Uuid::parse_str(&s).map_err(|_| CoreError::Invalid("uuid"))
}

fn dt_to_str(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

fn dt_from_str(s: String) -> Result<DateTime<Utc>, CoreError> {
    chrono::DateTime::parse_from_rfc3339(&s)
        .map_err(|_| CoreError::Invalid("datetime"))
        .map(|dt| dt.with_timezone(&Utc))
}

fn grade_to_i(g: &Grade) -> i64 {
    match g {
        Grade::Hard => 1,
        Grade::Medium => 2,
        Grade::Easy => 3,
    }
}

fn grade_from_i(i: i64) -> Option<Grade> {
    match i {
        1 => Some(Grade::Hard),
        2 => Some(Grade::Medium),
        3 => Some(Grade::Easy),
        _ => None,
    }
}

fn bool_to_i(b: bool) -> i64 {
    if b {
        1
    } else {
        0
    }
}

fn row_into_card(row: sqlx::sqlite::SqliteRow) -> Result<Card, CoreError> {
    let tags_json: String = row.get("tags");
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

    Ok(Card {
        id: uuid_from_str(row.get::<String, _>("id"))?,
        deck_id: uuid_from_str(row.get::<String, _>("deck_id"))?,
        front: row.get::<String, _>("front"),
        back: row.get::<String, _>("back"),
        hint: row.get::<Option<String>, _>("hint"),
        tags,
        reps: row.get::<i64, _>("reps") as u32,
        interval_days: row.get::<i64, _>("interval_days") as u32,
        ef: row.get::<f64, _>("ef") as f32,
        due_at: dt_from_str(row.get::<String, _>("due_at"))?,
        last_grade: row
            .get::<Option<i64>, _>("last_grade")
            .and_then(grade_from_i),
        last_reviewed_at: row
            .get::<Option<String>, _>("last_reviewed_at")
            .map(dt_from_str)
            .transpose()?,
        suspended: row.get::<i64, _>("suspended") != 0,
        created_at: dt_from_str(row.get::<String, _>("created_at"))?,
    })
}
