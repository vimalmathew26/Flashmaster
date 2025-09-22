use crate::cli::opts::*;
use crate::api::server as api_server;
use crate::tui::app::TuiApp;

use anyhow::{anyhow, bail, Result};
use chrono::Utc;
use flashmaster_core::{
    filters::{filter_by_due, filter_not_suspended},
    scheduler::apply_grade,
    DueStatus, Grade, Repository,
};
use flashmaster_core::{Card, Deck};
use flashmaster_json::paths::data_root;
use flashmaster_json::JsonStore;
use flashmaster_sqlite::SqliteRepo;
use std::io::{stdin, stdout, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use uuid::Uuid;

pub async fn run_cli(args: Cli) -> Result<()> {
    match &args.cmd {
        Command::Tui => {
            // (kept for completeness but main routes TUI directly)
            let repo = open_repo(&args.store, args.db_path.clone()).await?;
            let rt = Arc::new(Runtime::new()?);
            let mut app = TuiApp::new(repo, rt);
            app.run()?;
            Ok(())
        }
        Command::Api(api) => {
            let repo = open_repo(&args.store, args.db_path.clone()).await?;
            let addr: std::net::SocketAddr = api.addr.parse()?;
            api_server::run(repo, addr).await
        }
        _ => {
            let repo = open_repo(&args.store, args.db_path.clone()).await?;
            match args.cmd.clone() {
                Command::Deck(cmd) => deck_cmd(repo, cmd).await,
                Command::Card(cmd) => card_cmd(repo, cmd).await,
                Command::Review(cmd) => review_cmd(repo, cmd).await,
                Command::Export(cmd) => export_cmd(repo, cmd).await,
                Command::Import(cmd) => import_cmd(repo, cmd).await,
                _ => unreachable!(),
            }
        }
    }
}

pub async fn open_repo(store: &StoreKind, db_path: Option<PathBuf>) -> Result<Arc<dyn Repository>> {
    match store {
        StoreKind::Json => {
            let s = JsonStore::open_default().await?;
            Ok(Arc::new(s))
        }
        StoreKind::Sqlite => {
            let p = db_path.unwrap_or_else(|| data_root().join("flashmaster.sqlite3"));
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let s = SqliteRepo::open_file(&p).await?;
            Ok(Arc::new(s))
        }
    }
}

async fn deck_cmd(repo: Arc<dyn Repository>, cmd: DeckCmd) -> Result<()> {
    match cmd {
        DeckCmd::Add { name } => {
            let d = repo.create_deck(&name).await?;
            println!("{}", d.id);
        }
        DeckCmd::List => {
            let mut v = repo.list_decks().await?;
            v.sort_by_key(|d| d.created_at);
            for d in v {
                println!("{}\t{}", d.id, d.name);
            }
        }
        DeckCmd::Rm { deck } => {
            let d = resolve_deck(&*repo, &deck).await?;
            repo.delete_deck(d.id).await?;
            println!("ok");
        }
    }
    Ok(())
}

async fn card_cmd(repo: Arc<dyn Repository>, cmd: CardCmd) -> Result<()> {
    match cmd {
        CardCmd::Add(a) => {
            let deck = resolve_deck(&*repo, &a.deck).await?;
            let c = repo
                .add_card(deck.id, &a.front, &a.back, a.hint.as_deref(), &a.tags)
                .await?;
            println!("{}", c.id);
        }
        CardCmd::List { deck } => {
            let deck_id = if let Some(sel) = deck {
                Some(resolve_deck(&*repo, &sel).await?.id)
            } else {
                None
            };
            let mut cards = repo.list_cards(deck_id).await?;
            cards.sort_by_key(|c| c.created_at);
            for c in cards {
                let tags = if c.tags.is_empty() { "-".to_string() } else { c.tags.join(";") };
                println!("{}\t{}\t{}\tdeck={}\ttags={}\tsuspended={}", c.id, c.front, c.back, c.deck_id, tags, c.suspended);
            }
        }
        CardCmd::Rm { card_id } => {
            let id = parse_uuid(&card_id)?;
            repo.delete_card(id).await?;
            println!("ok");
        }
        CardCmd::Edit(e) => {
            let id = parse_uuid(&e.card_id)?;
            let mut card = repo.get_card(id).await?;

            if let Some(f) = e.front { card.front = f; }
            if let Some(b) = e.back { card.back = b; }
            if e.clear_hint { card.hint = None; }
            if let Some(h) = e.hint { card.hint = Some(h); }

            if !e.add_tags.is_empty() || !e.rm_tags.is_empty() {
                let mut tags = card.tags.clone();
                for t in e.add_tags { if !tags.iter().any(|x| x.eq_ignore_ascii_case(&t)) { tags.push(t); } }
                if !e.rm_tags.is_empty() {
                    tags.retain(|x| !e.rm_tags.iter().any(|r| x.eq_ignore_ascii_case(r)));
                }
                card.tags = tags;
            }

            if e.suspend && e.unsuspend {
                anyhow::bail!("cannot use --suspend and --unsuspend together");
            } else if e.suspend {
                card.suspended = true;
            } else if e.unsuspend {
                card.suspended = false;
            }

            let _ = repo.update_card(&card).await?;
            println!("ok");
        }
    }
    Ok(())
}

async fn review_cmd(repo: Arc<dyn Repository>, cmd: ReviewCmd) -> Result<()> {
    let now = Utc::now();

    let deck_filter = if let Some(sel) = cmd.deck {
        Some(resolve_deck(&*repo, &sel).await?.id)
    } else { None };

    let mut cards = repo.list_cards(deck_filter).await?;
    cards = filter_not_suspended(&cards);

    let mut pool = Vec::new();
    if cmd.include_new { pool.extend(filter_by_due(&cards, now, DueStatus::New)); }
    pool.extend(filter_by_due(&cards, now, DueStatus::DueToday));
    if cmd.include_lapsed { pool.extend(filter_by_due(&cards, now, DueStatus::Lapsed)); }

    pool.sort_by_key(|c| (c.due_at, c.created_at));
    if pool.is_empty() {
        println!("no cards due");
        return Ok(());
    }

    let mut count = 0usize;
    for mut card in pool.into_iter().take(cmd.max) {
        count += 1;
        println!("\n[{}/{}] {}", count, cmd.max, card.id);
        println!("Q: {}", card.front);
        prompt_enter("[enter=show]")?;
        println!("A: {}", card.back);
        if let Some(h) = &card.hint { println!("hint: {}", h); }
        println!("[1=Hard, 2=Medium, 3=Easy, s=skip, q=quit]");
        let g = loop {
            let line = read_line("grade> ")?;
            match line.trim().to_lowercase().as_str() {
                "1" | "h" | "hard" => break Some(Grade::Hard),
                "2" | "m" | "med" | "medium" => break Some(Grade::Medium),
                "3" | "e" | "easy" => break Some(Grade::Easy),
                "s" | "skip" => break None,
                "q" | "quit" => return Ok(()),
                _ => { println!("enter 1/2/3, s, or q"); }
            }
        };

        if let Some(grade) = g {
            let out = apply_grade(card, grade);
            repo.update_card(&out.updated_card).await?;
            repo.insert_review(&out.review).await?;
            card = out.updated_card;
            println!("→ next due in {} day(s)", card.interval_days);
        }
    }

    println!("\nreviewed {}", count);
    Ok(())
}

async fn export_cmd(repo: Arc<dyn Repository>, cmd: ExportCmd) -> Result<()> {
    match cmd {
        ExportCmd::Json { path } => {
            let decks = repo.list_decks().await?;
            let mut cards = repo.list_cards(None).await?;
            cards.sort_by_key(|c| c.created_at);
            let bundle = ExportBundle { version: 1, decks, cards };
            let s = serde_json::to_string_pretty(&bundle)?;
            std::fs::write(&path, s)?;
            println!("wrote {}", path.display());
        }
        ExportCmd::Csv { path, deck } => {
            let deck_id = if let Some(sel) = deck {
                Some(resolve_deck(&*repo, &sel).await?.id)
            } else { None };
            let mut cards = repo.list_cards(deck_id).await?;
            cards.sort_by_key(|c| c.created_at);

            let decks = repo.list_decks().await?;
            let mut deck_name: std::collections::HashMap<uuid::Uuid, String> =
                decks.into_iter().map(|d| (d.id, d.name)).collect();

            let mut wtr = csv::Writer::from_path(&path)?;
            wtr.write_record(["deck","front","back","hint","tags","suspended"])?;
            for c in cards {
                let dn = deck_name.remove(&c.deck_id).unwrap_or_else(|| c.deck_id.to_string());
                let tags = if c.tags.is_empty() { "".to_string() } else { c.tags.join(";") };
                wtr.write_record([
                    dn,
                    c.front,
                    c.back,
                    c.hint.unwrap_or_default(),
                    tags,
                    if c.suspended { "1".to_string() } else { "0".to_string() }
                ])?;
            }
            wtr.flush()?;
            println!("wrote {}", path.display());
        }
    }
    Ok(())
}

async fn import_cmd(repo: Arc<dyn Repository>, cmd: ImportCmd) -> Result<()> {
    match cmd {
        ImportCmd::Json { path } => {
            let data = std::fs::read_to_string(&path)?;
            let bundle: ExportBundle = serde_json::from_str(&data)?;
            for d in bundle.decks { let _ = repo.create_deck(&d.name).await; }
            let decks = repo.list_decks().await?;
            for c in bundle.cards {
                let deck = resolve_deck(&*repo, &select_deck_by_id_or_name(&decks, c.deck_id, None)).await?;
                let _ = repo.add_card(deck.id, &c.front, &c.back, c.hint.as_deref(), &c.tags).await?;
            }
            println!("imported");
        }
        ImportCmd::Csv { path, deck } => {
            let mut rdr = csv::Reader::from_path(&path)?;
            let mut target_deck = None;
            if let Some(sel) = deck { target_deck = Some(resolve_deck(&*repo, &sel).await?); }
            for rec in rdr.records() {
                let rec = rec?;
                let deck_name = rec.get(0).unwrap_or("").trim();
                let front = rec.get(1).unwrap_or("").to_string();
                let back  = rec.get(2).unwrap_or("").to_string();
                let hint  = rec.get(3).map(|s| s.to_string()).filter(|s| !s.is_empty());
                let tags  = rec.get(4).unwrap_or("").split(';').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect::<Vec<_>>();
                let suspended = rec.get(5).unwrap_or("0").trim() == "1";

                let deck_obj = if let Some(d) = &target_deck { d.clone() } else { ensure_deck_by_name(&*repo, deck_name).await? };
                let card = repo.add_card(deck_obj.id, &front, &back, hint.as_deref(), &tags).await?;
                if suspended { repo.set_suspended(card.id, true).await?; }
            }
            println!("imported");
        }
    }
    Ok(())
}

// ===== Helpers =====
fn parse_uuid(s: &str) -> Result<uuid::Uuid> { Uuid::parse_str(s).map_err(|_| anyhow!("invalid uuid")) }

async fn resolve_deck<R: Repository + ?Sized>(repo: &R, sel: &str) -> Result<Deck> {
    if let Ok(id) = Uuid::parse_str(sel) { if let Ok(d) = repo.get_deck(id).await { return Ok(d); } }
    let decks = repo.list_decks().await?;
    if let Some(d) = decks.into_iter().find(|d| d.name.eq_ignore_ascii_case(sel)) { return Ok(d); }
    bail!("deck not found: {}", sel)
}

async fn ensure_deck_by_name<R: Repository + ?Sized>(repo: &R, name: &str) -> Result<Deck> {
    let decks = repo.list_decks().await?;
    if let Some(d) = decks.into_iter().find(|d| d.name.eq_ignore_ascii_case(name)) { return Ok(d); }
    let d = repo.create_deck(name).await?;
    Ok(d)
}

fn prompt_enter(label: &str) -> Result<()> { print!("{label}"); stdout().flush().ok(); let mut s = String::new(); stdin().read_line(&mut s)?; Ok(()) }
fn read_line(prompt: &str) -> Result<String> { print!("{prompt}"); stdout().flush().ok(); let mut s = String::new(); stdin().read_line(&mut s)?; Ok(s) }

#[derive(serde::Serialize, serde::Deserialize)]
struct ExportBundle { version: u32, decks: Vec<Deck>, cards: Vec<Card> }

fn select_deck_by_id_or_name(decks: &[Deck], id: uuid::Uuid, name: Option<String>) -> String {
    if let Some(d) = decks.iter().find(|d| d.id == id) { d.name.clone() } else if let Some(n) = name { n } else { id.to_string() }
}
