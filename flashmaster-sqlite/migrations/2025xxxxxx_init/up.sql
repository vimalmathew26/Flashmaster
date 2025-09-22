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
