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

    /// PR number or URL to pre-select (e.g., 123 or https://github.com/owner/repo/pull/123)
    #[arg(long)]
    pr: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Parse PR argument - can be number or URL
    let (repo_from_pr, pr_number) = parse_pr_arg(&args.pr);

    // Detect repo from git remote if not provided
    let repo = match args.repo {
        Some(r) => r,
        None => repo_from_pr.unwrap_or_else(|| {
            detect_repo().unwrap_or_else(|| "shopsys/shopsys".to_string())
        }),
    };

    // Setup terminal
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // Create and run app
    let mut app = app::App::new(repo);
    app.initial_pr = pr_number;
    let result = app.run(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;

    if let Err(ref e) = result {
        eprintln!("Error: {}", e);
    }

    result
}

/// Parse PR argument which can be a number or a GitHub PR URL
/// Returns (optional_repo, optional_pr_number)
fn parse_pr_arg(pr_arg: &Option<String>) -> (Option<String>, Option<u64>) {
    let pr_str = match pr_arg {
        Some(s) => s,
        None => return (None, None),
    };

    // Try parsing as a number first
    if let Ok(num) = pr_str.parse::<u64>() {
        return (None, Some(num));
    }

    // Try parsing as a GitHub PR URL
    // Format: https://github.com/owner/repo/pull/123
    if pr_str.contains("github.com") && pr_str.contains("/pull/") {
        let parts: Vec<&str> = pr_str.split('/').collect();
        // Find the index of "pull" and get the number after it
        if let Some(pull_idx) = parts.iter().position(|&p| p == "pull") {
            if let Some(num_str) = parts.get(pull_idx + 1) {
                if let Ok(num) = num_str.parse::<u64>() {
                    // Extract owner/repo
                    if let (Some(owner_idx), Some(repo_idx)) = (
                        parts.iter().position(|&p| p == "github.com").map(|i| i + 1),
                        parts.iter().position(|&p| p == "github.com").map(|i| i + 2),
                    ) {
                        if let (Some(owner), Some(repo)) = (parts.get(owner_idx), parts.get(repo_idx)) {
                            return (Some(format!("{}/{}", owner, repo)), Some(num));
                        }
                    }
                    return (None, Some(num));
                }
            }
        }
    }

    (None, None)
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
