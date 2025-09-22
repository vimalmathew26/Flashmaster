use clap::{Parser, Subcommand, Args, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, ValueEnum)]
pub enum StoreKind {
    Json,
    Sqlite,
}

#[derive(Debug, Parser, Clone)]
#[command(name = "flashmaster", version, about = "FlashMaster CLI/TUI/API")]
pub struct Cli {
    /// Storage backend (applies to CLI/TUI/API unless overridden)
    #[arg(long, value_enum, default_value_t = StoreKind::Json)]
    pub store: StoreKind,

    /// SQLite DB path when --store sqlite (defaults to app data dir)
    #[arg(long)]
    pub db_path: Option<PathBuf>,

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, Subcommand, Clone)]
pub enum Command {
    /// Deck operations (CLI)
    #[command(subcommand)]
    Deck(DeckCmd),
    /// Card operations (CLI)
    #[command(subcommand)]
    Card(CardCmd),
    /// Review loop (CLI)
    Review(ReviewCmd),
    /// Export data (CLI)
    #[command(subcommand)]
    Export(ExportCmd),
    /// Import data (CLI)
    #[command(subcommand)]
    Import(ImportCmd),
    /// Launch Terminal UI
    Tui,
    /// Launch Axum HTTP API
    Api(ApiCmd),
}

#[derive(Debug, Subcommand, Clone)]
pub enum DeckCmd {
    Add { name: String },
    List,
    Rm { deck: String },
}

#[derive(Debug, Subcommand, Clone)]
pub enum CardCmd {
    Add(CardAdd),
    List { #[arg(long)] deck: Option<String> },
    Rm { card_id: String },
    Edit(CardEdit),
}

#[derive(Debug, Args, Clone)]
pub struct CardAdd {
    #[arg(long)]
    pub deck: String,
    #[arg(long)]
    pub front: String,
    #[arg(long)]
    pub back: String,
    #[arg(long)]
    pub hint: Option<String>,
    #[arg(long = "tag")]
    pub tags: Vec<String>,
}

#[derive(Debug, Args, Clone)]
pub struct CardEdit {
    pub card_id: String,
    #[arg(long)]
    pub front: Option<String>,
    #[arg(long)]
    pub back: Option<String>,
    #[arg(long)]
    pub hint: Option<String>,
    #[arg(long)]
    pub clear_hint: bool,
    #[arg(long = "add-tag")]
    pub add_tags: Vec<String>,
    #[arg(long = "rm-tag")]
    pub rm_tags: Vec<String>,
    #[arg(long)]
    pub suspend: bool,
    #[arg(long)]
    pub unsuspend: bool,
}

#[derive(Debug, Args, Clone)]
pub struct ReviewCmd {
    #[arg(long)]
    pub deck: Option<String>,
    #[arg(long)]
    pub include_new: bool,
    #[arg(long)]
    pub include_lapsed: bool,
    #[arg(long, default_value_t = 50)]
    pub max: usize,
}

#[derive(Debug, Subcommand, Clone)]
pub enum ExportCmd {
    Json { path: PathBuf },
    Csv { path: PathBuf, #[arg(long)] deck: Option<String> },
}

#[derive(Debug, Subcommand, Clone)]
pub enum ImportCmd {
    Json { path: PathBuf },
    Csv { path: PathBuf, #[arg(long)] deck: Option<String> },
}

#[derive(Debug, Args, Clone)]
pub struct ApiCmd {
    /// Bind address (host:port)
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub addr: String,
}
