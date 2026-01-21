# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

GitHub TUI is a terminal user interface for GitHub workflows built in Rust using Ratatui. It provides keyboard-driven management of pull requests, GitHub Actions workflows, and logs.

## Build Commands

```bash
cargo build              # Development build
cargo build --release    # Optimized release build (LTO enabled)
cargo run                # Run directly
cargo run -- --repo owner/repo --pr 123  # Run with arguments
```

The release profile uses LTO, single codegen unit, and panic=abort for a smaller binary.

## Architecture

### Core Components

- **app.rs** - Central application state, event loop, async task management (~1600 lines). This is the heart of the application.
- **event.rs** - Async event handler that spawns a background task emitting Tick, Key, and Resize events via mpsc channel.
- **github/client.rs** - Hybrid GitHub API client using Octocrab for structured calls and `gh` CLI for diffs/logs.
- **github/types.rs** - Data structures (PullRequest, WorkflowRun, Job, Commit) with helper methods.
- **ui/*.rs** - Component-based rendering, each file handles one UI section.

### State Management

The app uses a state machine pattern with key enums:
- `Tab` (PRs, Actions, Logs) - Which tab is active
- `View` (List, Detail, Diff, Jobs) - Current view within tab
- `Focus` (List, Detail, PrChecks) - Which panel has keyboard focus
- `InputMode` (Search, Comment, EditTitle, AddLabel, AddReviewer) - Text input modes
- `StatusMessage` - Auto-dismissing notifications vs persistent prompts

### Async Pattern

All data fetches are non-blocking. The pattern used throughout:
```rust
fn spawn_fetch_*(&self) {
    let tx = self.async_tx.clone();
    tokio::spawn(async move {
        // ... async operation
        tx.send(AsyncMsg::*Loaded(data)).ok();
    });
}
```
Results are processed in the main loop via `process_async_messages()`.

### UI Rendering

Layout hierarchy: Header (tabs) → Content (tab-specific) → Footer (context-sensitive help)

Each component has its own render function in `ui/`. The main orchestration happens in `ui/render.rs`.

## Key Patterns

### Adding a New Feature

1. Add state to `App` struct in `app.rs`
2. Add key handling in `handle_*_keys()` methods
3. Add UI rendering in appropriate `ui/*.rs` file
4. If async data needed, add `AsyncMsg` variant and `spawn_fetch_*` method

### Status Messages

Use the `StatusMessage` enum for user feedback:
```rust
// Auto-dismissing notification (3 seconds)
self.set_message("Action completed");

// Persistent prompt (for input modes)
self.status_message = Some(StatusMessage::prompt("Enter value:"));
```

### Authentication

Token resolution order:
1. `GITHUB_TOKEN` env var
2. `GH_TOKEN` env var
3. `gh auth token` CLI command

## CLI Arguments

```bash
--repo owner/repo    # Specify repository (auto-detects from git remote if omitted)
--pr NUMBER_OR_URL   # Pre-select a PR (supports both "123" and full GitHub URL)
```
