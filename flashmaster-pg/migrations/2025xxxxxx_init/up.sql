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
