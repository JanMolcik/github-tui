# CLAUDE.md

## User Context

**The user is a beginner with Rust.** Explain Rust-specific concepts when relevant.

## Tech Stack

- **Ratatui** - TUI framework
- **Crossterm** - Terminal backend
- **Tokio** - Async runtime
- **Octocrab** - GitHub API client
- **gh CLI** - Used for diffs, logs, and some PR operations

## Architecture

```
src/
├── main.rs          # Entry point, terminal setup
├── app.rs           # Application state, event loop, key handling (~1600 lines)
├── event.rs         # Async event handler (Tick, Key, Resize via mpsc)
├── ui/              # Component-based rendering
│   ├── render.rs    # Main orchestration
│   ├── pr_list.rs, pr_detail.rs, actions_list.rs, jobs_view.rs, log_viewer.rs
│   └── styles.rs, help.rs
└── github/
    ├── client.rs    # Hybrid API: Octocrab + gh CLI
    └── types.rs     # PullRequest, WorkflowRun, Job, Commit
```

State machine pattern: `Tab` → `View` → `Focus` → `InputMode`

## Build Commands

```bash
cargo build --release    # ALWAYS use --release (user runs ./target/release/github-tui)
cargo run --release      # Run directly
cargo clippy --fix --allow-dirty  # Fix lints
cargo test               # Run tests
```

**IMPORTANT:** Always use `--release` flag. Debug builds are not used.

## Key Patterns

### Adding Features

1. Add state to `App` struct in `app.rs`
2. Add key handling in `handle_*_keys()` methods
3. Add UI in appropriate `ui/*.rs` file
4. For async data: add `AsyncMsg` variant + `spawn_fetch_*` method

### Status Messages

```rust
// Auto-dismissing (3 sec)
self.set_message("Done");

// Persistent prompt
self.status_message = Some(StatusMessage::prompt("Enter value:"));
```

## Debugging TUI Apps

TUI takes over terminal - use file logging:

```rust
let file = std::fs::File::create("/tmp/app.log").unwrap();
tracing_subscriber::fmt().with_writer(file).init();
// Then: tail -f /tmp/app.log
```

## Gotchas

- Auth uses: `GITHUB_TOKEN` env → `GH_TOKEN` env → `gh auth token` CLI
- Diff/logs fetched via `gh` CLI (not Octocrab API)
- `.env.local` contains token - keep permissions at 600
