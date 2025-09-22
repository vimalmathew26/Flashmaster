
# FlashMaster

FlashMaster is a local-first flashcard app written in Rust with a lightweight spaced-repetition scheduler (SM-2-lite), a pleasant TUI, a simple CLI, and an optional HTTP API. Storage backends include JSON (default) and SQLite.

> Quality over speed. The code is structured as a small workspace with clear boundaries between domain logic and persistence.

---

## Contents

- [Features](#features)
- [Architecture](#architecture)
- [Prerequisites](#prerequisites)
- [Setup](#setup)
- [Build, Lint, Test](#build-lint-test)
- [Run (CLI)](#run-cli)
- [Run (TUI)](#run-tui)
- [Run (HTTP API)](#run-http-api)
- [Import / Export](#import--export)
- [Storage & Data Locations](#storage--data-locations)
- [Project Layout](#project-layout)
- [Scheduler Notes (SM-2-lite)](#scheduler-notes-sm2-lite)
- [Troubleshooting](#troubleshooting)
- [License](#license)

---

## Features

- **Decks & Cards**: create/edit/delete decks and cards (front/back, optional hint, tags).
- **Spaced Repetition**: SM-2-lite scheduling with three grades: Hard, Medium, Easy.
- **Due Queue**: study Today’s Due; optionally include New and Lapsed; caps and ordering.
- **Search/Filter (core)**: filter by due status, text, and tag.
- **Stats (core)**: daily totals, accuracy, per-deck aggregates.
- **TUI**: keyboard-driven review loop with reveal and quick grading.
- **CLI**: manage decks/cards, run reviews, import/export.
- **HTTP API (Axum)**: minimal JSON endpoints to list decks, get due cards, and post reviews.
- **Persistence**:
  - **JSON** (default): atomic writes with timestamped, rotating backups.
  - **SQLite**: embedded DB via `sqlx` (bundled libsqlite3).
- **Cross-platform**: Windows, Linux, macOS.

---

## Architecture

Workspace crates:

- `flashmaster-core` — domain models, scheduler, filters, stats, and a repository trait.
- `flashmaster-json` — JSON store with atomic writes and rotating backups.
- `flashmaster-sqlite` — SQLite repo implemented with `sqlx` (bundled).
- `flashmaster-pg` — PostgreSQL repo (implemented, not wired into the default binary).
- `flashmaster-app` — CLI/TUI/API binary (select storage backend at runtime).

---

## Prerequisites

### All platforms
- Rust (stable). Recommended components: `rustfmt`, `clippy`.
- Git.

### Windows 11
- **Rust (MSVC toolchain)**: install via [rustup](https://rustup.rs/) or `winget install Rustlang.Rust.MSVC`.
- **Build tools**: Visual Studio Build Tools (C++ workload) for native crates.

### macOS
- Xcode Command Line Tools: `xcode-select --install`
- Rust via rustup.

### Linux
- Build essentials: `sudo apt-get install build-essential pkg-config` (Debian/Ubuntu) or equivalents.

> SQLite is bundled; no system SQLite required.

---

## Setup

```bash
git clone https://github.com/<you>/flashmaster.git
cd flashmaster

# Optional but recommended:
rustup component add rustfmt clippy
````

---

## Build, Lint, Test

```bash
# Format, lint (deny warnings), tests
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace

# Build everything
cargo build --workspace

# Release build of the app
cargo build -p flashmaster-app --release
```

CI is provided via GitHub Actions (`.github/workflows/ci.yml`) on Linux and Windows.

---

## Run (CLI)

The binary lives in `flashmaster-app`. JSON store is the default.

```bash
# Show help
cargo run -p flashmaster-app -- --help

# Create a deck and cards
cargo run -p flashmaster-app -- deck add "Spanish"
cargo run -p flashmaster-app -- card add --deck Spanish --front hola   --back hello   --tag greeting --tag spanish
cargo run -p flashmaster-app -- card add --deck Spanish --front gracias --back thanks  --tag spanish

# List
cargo run -p flashmaster-app -- deck list
cargo run -p flashmaster-app -- card list --deck Spanish

# Review (include new cards)
cargo run -p flashmaster-app -- review --deck Spanish --include-new
```

### CLI with SQLite

```bash
DB=./flashmaster.sqlite3
cargo run -p flashmaster-app -- --store sqlite --db-path "$DB" deck add "Chemistry"
cargo run -p flashmaster-app -- --store sqlite --db-path "$DB" card add --deck Chemistry --front "H2O" --back "Water"
cargo run -p flashmaster-app -- --store sqlite --db-path "$DB" review --deck Chemistry --include-new
```

---

## Run (TUI)

```bash
cargo run -p flashmaster-app -- tui
```

**Keys**

* Navigation: `Up/k`, `Down/j`
* Start review: `Enter`
* Reveal: `Space`
* Grade: `1` = Hard, `2` = Medium, `3` = Easy
* Skip: `s`
* Quit: `q`

---

## Run (HTTP API)

Start the server:

```bash
cargo run -p flashmaster-app -- api --addr 127.0.0.1:8080
# or with SQLite
# cargo run -p flashmaster-app -- --store sqlite --db-path ./flashmaster.sqlite3 api --addr 127.0.0.1:8080
```

Endpoints:

* `GET /decks` — list decks
* `GET /due?deck=<name-or-uuid>&include_new=true&include_lapsed=true&max=50` — due cards
* `POST /review` — apply a review

Example:

```bash
# list decks
curl http://127.0.0.1:8080/decks

# fetch due cards
curl "http://127.0.0.1:8080/due?deck=Spanish&include_new=true&include_lapsed=true&max=20"

# post a review
curl -X POST http://127.0.0.1:8080/review \
  -H "Content-Type: application/json" \
  -d '{"card_id":"<CARD_UUID>","grade":"easy"}'
```

---

## Import / Export

### Export

```bash
# JSON bundle (all decks)
cargo run -p flashmaster-app -- export json --path ./backup.json

# CSV (optionally restrict to one deck)
cargo run -p flashmaster-app -- export csv --path ./spanish.csv --deck Spanish
```

CSV columns (header row included):

```
deck,front,back,hint,tags,suspended
```

* `tags`: semicolon-separated list, e.g. `greeting;spanish`
* `suspended`: `1` or `0`

### Import

```bash
# JSON bundle
cargo run -p flashmaster-app -- import json --path ./backup.json

# CSV (if --deck is provided, all rows are imported into that deck,
# otherwise the first column "deck" is used per row)
cargo run -p flashmaster-app -- import csv --path ./spanish.csv --deck Spanish
```

---

## Storage & Data Locations

### JSON store (default)

* **Windows**: `%APPDATA%\com\flashmaster\FlashMaster\flashmaster.json`
  Backups in `%APPDATA%\com\flashmaster\FlashMaster\backups\` (timestamped rotation).
* **macOS**: `~/Library/Application Support/com/flashmaster/FlashMaster/flashmaster.json`
* **Linux**: `~/.local/share/com/flashmaster/FlashMaster/flashmaster.json`

### SQLite

You choose the path with `--db-path`. If omitted, a sensible location under the platform data directory is used.

---

## Project Layout

```
flashmaster/
├─ Cargo.toml
├─ README.md  LICENSE  .gitignore  .env.example
├─ .github/workflows/ci.yml
├─ flashmaster-core/
│  ├─ Cargo.toml
│  └─ src/
│     ├─ lib.rs
│     ├─ models.rs
│     ├─ scheduler.rs
│     ├─ filters.rs
│     ├─ stats.rs
│     └─ errors.rs
│  └─ tests/
│     ├─ scheduler_tests.rs
│     └─ stats_and_filters_tests.rs
├─ flashmaster-json/
│  ├─ Cargo.toml
│  └─ src/{lib.rs,paths.rs}
├─ flashmaster-sqlite/
│  ├─ Cargo.toml
│  ├─ migrations/2025xxxxxx_init/{up.sql,down.sql}
│  └─ src/lib.rs
├─ flashmaster-pg/
│  ├─ Cargo.toml
│  ├─ migrations/2025xxxxxx_init/{up.sql,down.sql}
│  └─ src/lib.rs
└─ flashmaster-app/
   ├─ Cargo.toml
   └─ src/
      ├─ main.rs
      ├─ cli/{mod.rs,opts.rs,commands.rs}
      ├─ tui/{mod.rs,app.rs,views.rs,inputs.rs,theme.rs}
      └─ api/{mod.rs,server.rs,routes.rs,dto.rs}
```

---

## Scheduler Notes (SM-2-lite)

* **Grades**: `Hard`, `Medium`, `Easy` (mapped to 1/2/3).
* **Ease factor (EF)** is adjusted each review and clamped to a safe range.
* **Intervals**:

  * First correct: 1 day
  * Second correct: 6 days
  * Subsequent: `round(prev_interval * EF)` with a minimum of 1 day
  * `Hard` resets repetitions and returns to a 1-day interval
* This yields a pragmatic, easy-to-understand progression suitable for small to mid-size decks.

---

## Troubleshooting

* **Windows build tools**: ensure Visual Studio Build Tools (C++ workload) are installed for native crates.
* **SQLite linking conflicts**: the workspace pins `libsqlite3-sys = 0.26.0` with `bundled` to avoid multiple `sqlite3` linkers. If you add crates that also link SQLite, keep versions consistent.
* **Port already in use**: when starting the API, change `--addr` or free the port.
* **Terminal issues**: if the TUI leaves the terminal in an odd state after a crash, run `reset` (Linux/macOS) or close/reopen the terminal (Windows).


