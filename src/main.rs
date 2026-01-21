use anyhow::Result;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io::stdout;

mod app;
mod event;
mod github;
mod ui;

#[derive(Parser, Debug)]
#[command(author, version, about = "A terminal UI for GitHub workflows")]
struct Args {
    /// Repository in format owner/repo
    #[arg(short, long)]
    repo: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Detect repo from git remote if not provided
    let repo = match args.repo {
        Some(r) => r,
        None => detect_repo().unwrap_or_else(|| "shopsys/shopsys".to_string()),
    };

    // Setup terminal
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // Create and run app
    let mut app = app::App::new(repo);
    let result = app.run(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;

    if let Err(ref e) = result {
        eprintln!("Error: {}", e);
    }

    result
}

fn detect_repo() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let url = String::from_utf8(output.stdout).ok()?;
    let url = url.trim();

    // Parse GitHub URL formats:
    // git@github.com:owner/repo.git
    // https://github.com/owner/repo.git
    if url.contains("github.com") {
        let repo = url
            .trim_start_matches("git@github.com:")
            .trim_start_matches("https://github.com/")
            .trim_end_matches(".git");
        Some(repo.to_string())
    } else {
        None
    }
}
