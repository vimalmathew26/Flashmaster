mod cli;
pub mod tui;
pub mod api;

use anyhow::Result;
use clap::Parser; // needed for Cli::parse()
use std::sync::Arc;
use tokio::runtime::Runtime;

use cli::opts::{Cli, Command};
use cli::commands::{run_cli, open_repo};
use tui::app::TuiApp;

fn main() -> Result<()> {
    let args = Cli::parse();

    match &args.cmd {
        // Run TUI on its own thread/runtime (no nested Tokio)
        Command::Tui => {
            let rt = Arc::new(Runtime::new()?);
            let repo = rt.block_on(open_repo(&args.store, args.db_path.clone()))?;
            let mut app = TuiApp::new(repo, rt);
            app.run()
        }
        // Everything else uses a single runtime here
        _ => {
            let rt = Runtime::new()?;
            rt.block_on(run_cli(args))
        }
    }
}
