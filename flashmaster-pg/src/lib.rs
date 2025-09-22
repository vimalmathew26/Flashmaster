use chrono::{DateTime, Utc};
use flashmaster_core::{repo::Repository, Card, CardId, CoreError, Deck, DeckId, Grade, Review};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};

pub struct PostgresRepo {
    pool: PgPool,
}

impl PostgresRepo {
    pub async fn connect(url: &str) -> Result<Self, CoreError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await
            .map_err(|_| CoreError::Storage("pg connect"))?;
        let repo = Self { pool };
        repo.ensure_schema().await?;
        Ok(repo)
    }

    async fn ensure_schema(&self) -> Result<(), CoreError> {
        // Mirrors migrations (id generation done in app; DB defaults still helpful)
        const STMT: &str = r#"
        CREATE EXTENSION IF NOT EXISTS "pgcrypto";

        CREATE TABLE IF NOT EXISTS decks (
          id          uuid PRIMARY KEY,
          name        text NOT NULL UNIQUE,
          created_at  timestamptz NOT NULL
        );

        CREATE TABLE IF NOT EXISTS cards (
          id                uuid PRIMARY KEY,
          deck_id           uuid NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
          front             text NOT NULL,
          back              text NOT NULL,
          hint              text,
          tags              text[] NOT NULL DEFAULT '{}',
          reps              integer NOT NULL DEFAULT 0,
          interval_days     integer NOT NULL DEFAULT 0,
          ef                real    NOT NULL DEFAULT 2.5,
          due_at            timestamptz NOT NULL,
          last_grade        smallint,
          last_reviewed_at  timestamptz,
          suspended         boolean NOT NULL DEFAULT false,
          created_at        timestamptz NOT NULL
        );

        CREATE TABLE IF NOT EXISTS reviews (
          id               uuid PRIMARY KEY,
          card_id          uuid NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
          grade            smallint NOT NULL,
          reviewed_at      timestamptz NOT NULL,
          interval_applied integer NOT NULL,
          ef_after         real NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_cards_deck_due ON cards (deck_id, due_at);
        CREATE INDEX IF NOT EXISTS idx_reviews_card_time ON reviews (card_id, reviewed_at);
        "#;

        for chunk in STMT.split(';') {
            let sql = chunk.trim();
            if sql.is_empty() {
                continue;
            }
            sqlx::query(sql)
                .execute(&self.pool)
                .await
                .map_err(|_| CoreError::Storage("pg schema"))?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl Repository for PostgresRepo {
    // ===== Decks =====
    async fn create_deck(&self, name: &str) -> Result<Deck, CoreError> {
        // unique name pre-check
        let exists =
            sqlx::query_scalar::<_, i64>("SELECT 1 FROM decks WHERE lower(name)=lower($1) LIMIT 1")
                .bind(name)
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| CoreError::Storage("pg read deck"))?
                .is_some();
        if exists {
            return Err(CoreError::Conflict("deck name already exists"));
        }

        let deck = Deck::new(name);
        sqlx::query("INSERT INTO decks (id,name,created_at) VALUES ($1,$2,$3)")
            .bind(deck.id)
            .bind(&deck.name)
            .bind(deck.created_at)
            .execute(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("pg insert deck"))?;
        Ok(deck)
    }

    async fn get_deck(&self, id: DeckId) -> Result<Deck, CoreError> {
        let row = sqlx::query("SELECT id,name,created_at FROM decks WHERE id=$1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("pg read deck"))?;
        let row = row.ok_or(CoreError::NotFound("deck"))?;
        Ok(Deck {
            id: row.get::<uuid::Uuid, _>("id"),
            name: row.get::<String, _>("name"),
            created_at: row.get::<DateTime<Utc>, _>("created_at"),
        })
    }

    async fn list_decks(&self) -> Result<Vec<Deck>, CoreError> {
        let rows = sqlx::query("SELECT id,name,created_at FROM decks ORDER BY created_at ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("pg list decks"))?;
        Ok(rows
            .into_iter()
            .map(|row| Deck {
                id: row.get("id"),
                name: row.get("name"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    async fn delete_deck(&self, id: DeckId) -> Result<(), CoreError> {
        let res = sqlx::query("DELETE FROM decks WHERE id=$1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("pg del deck"))?;
        if res.rows_affected() == 0 {
            return Err(CoreError::NotFound("deck"));
        }
        Ok(())
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
        // ensure deck exists
        let exists = sqlx::query_scalar::<_, i64>("SELECT 1 FROM decks WHERE id=$1 LIMIT 1")
            .bind(deck_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("pg read deck"))?
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
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
            "#,
        )
        .bind(card.id)
        .bind(card.deck_id)
        .bind(&card.front)
        .bind(&card.back)
        .bind(card.hint.clone())
        .bind(&card.tags) // text[]
        .bind(card.reps as i64)
        .bind(card.interval_days as i64)
        .bind(card.ef as f64)
        .bind(card.due_at)
        .bind(card.last_grade.as_ref().map(grade_to_i16))
        .bind(card.last_reviewed_at)
        .bind(card.suspended)
        .bind(card.created_at)
        .execute(&self.pool)
        .await
        .map_err(|_| CoreError::Storage("pg insert card"))?;

        Ok(card)
    }

    async fn get_card(&self, id: CardId) -> Result<Card, CoreError> {
        let row = sqlx::query(
            r#"SELECT id,deck_id,front,back,hint,tags,reps,interval_days,ef,due_at,
                       last_grade,last_reviewed_at,suspended,created_at
               FROM cards WHERE id=$1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CoreError::Storage("pg read card"))?;
        let row = row.ok_or(CoreError::NotFound("card"))?;
        row_into_card(row)
    }

    async fn list_cards(&self, deck_id: Option<DeckId>) -> Result<Vec<Card>, CoreError> {
        let rows = if let Some(did) = deck_id {
            sqlx::query(
                r#"SELECT id,deck_id,front,back,hint,tags,reps,interval_days,ef,due_at,
                          last_grade,last_reviewed_at,suspended,created_at
                   FROM cards WHERE deck_id=$1 ORDER BY created_at ASC"#,
            )
            .bind(did)
            .fetch_all(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("pg list cards"))?
        } else {
            sqlx::query(
                r#"SELECT id,deck_id,front,back,hint,tags,reps,interval_days,ef,due_at,
                          last_grade,last_reviewed_at,suspended,created_at
                   FROM cards ORDER BY created_at ASC"#,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("pg list cards"))?
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
              deck_id=$1, front=$2, back=$3, hint=$4, tags=$5, reps=$6, interval_days=$7,
              ef=$8, due_at=$9, last_grade=$10, last_reviewed_at=$11, suspended=$12
            WHERE id=$13
            "#,
        )
        .bind(card.deck_id)
        .bind(&card.front)
        .bind(&card.back)
        .bind(card.hint.clone())
        .bind(&card.tags)
        .bind(card.reps as i64)
        .bind(card.interval_days as i64)
        .bind(card.ef as f64)
        .bind(card.due_at)
        .bind(card.last_grade.as_ref().map(grade_to_i16))
        .bind(card.last_reviewed_at)
        .bind(card.suspended)
        .bind(card.id)
        .execute(&self.pool)
        .await
        .map_err(|_| CoreError::Storage("pg update card"))?;
        if res.rows_affected() == 0 {
            return Err(CoreError::NotFound("card"));
        }
        Ok(card.clone())
    }

    async fn delete_card(&self, id: CardId) -> Result<(), CoreError> {
        let res = sqlx::query("DELETE FROM cards WHERE id=$1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("pg del card"))?;
        if res.rows_affected() == 0 {
            return Err(CoreError::NotFound("card"));
        }
        Ok(())
    }

    async fn set_suspended(&self, id: CardId, suspended: bool) -> Result<(), CoreError> {
        let res = sqlx::query("UPDATE cards SET suspended=$1 WHERE id=$2")
            .bind(suspended)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|_| CoreError::Storage("pg suspend"))?;
        if res.rows_affected() == 0 {
            return Err(CoreError::NotFound("card"));
        }
        Ok(())
    }

    // ===== Reviews =====
    async fn insert_review(&self, review: &Review) -> Result<(), CoreError> {
        sqlx::query(
            r#"INSERT INTO reviews (id,card_id,grade,reviewed_at,interval_applied,ef_after)
               VALUES ($1,$2,$3,$4,$5,$6)"#,
        )
        .bind(review.id)
        .bind(review.card_id)
        .bind(grade_to_i16(&review.grade))
        .bind(review.reviewed_at)
        .bind(review.interval_applied as i64)
        .bind(review.ef_after as f64)
        .execute(&self.pool)
        .await
        .map_err(|_| CoreError::Storage("pg insert review"))?;
        Ok(())
    }

    async fn list_reviews_for_card(&self, card_id: CardId) -> Result<Vec<Review>, CoreError> {
        let rows = sqlx::query(
            r#"SELECT id,card_id,grade,reviewed_at,interval_applied,ef_after
               FROM reviews WHERE card_id=$1 ORDER BY reviewed_at ASC"#,
        )
        .bind(card_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CoreError::Storage("pg list reviews"))?;
        let mut v = Vec::with_capacity(rows.len());
        for row in rows {
            v.push(Review {
                id: row.get::<uuid::Uuid, _>("id"),
                card_id: row.get::<uuid::Uuid, _>("card_id"),
                grade: grade_from_i16(row.get::<i16, _>("grade"))
                    .ok_or(CoreError::Invalid("grade"))?,
                reviewed_at: row.get::<DateTime<Utc>, _>("reviewed_at"),
                interval_applied: row.get::<i32, _>("interval_applied"),
                ef_after: row.get::<f32, _>("ef_after"),
            });
        }
        Ok(v)
    }
}

// ===== helpers =====
fn grade_to_i16(g: &Grade) -> i16 {
    match g {
        Grade::Hard => 1,
        Grade::Medium => 2,
        Grade::Easy => 3,
    }
}

fn grade_from_i16(i: i16) -> Option<Grade> {
    match i {
        1 => Some(Grade::Hard),
        2 => Some(Grade::Medium),
        3 => Some(Grade::Easy),
        _ => None,
    }
}

fn row_into_card(row: sqlx::postgres::PgRow) -> Result<Card, CoreError> {
    Ok(Card {
        id: row.get::<uuid::Uuid, _>("id"),
        deck_id: row.get::<uuid::Uuid, _>("deck_id"),
        front: row.get::<String, _>("front"),
        back: row.get::<String, _>("back"),
        hint: row.get::<Option<String>, _>("hint"),
        tags: row.get::<Vec<String>, _>("tags"),
        reps: row.get::<i32, _>("reps") as u32,
        interval_days: row.get::<i32, _>("interval_days") as u32,
        ef: row.get::<f32, _>("ef"),
        due_at: row.get::<DateTime<Utc>, _>("due_at"),
        last_grade: row
            .get::<Option<i16>, _>("last_grade")
            .and_then(grade_from_i16),
        last_reviewed_at: row.get::<Option<DateTime<Utc>>, _>("last_reviewed_at"),
        suspended: row.get::<bool, _>("suspended"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
    })
}
