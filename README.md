# GitHub TUI

A fast terminal UI for GitHub workflows built in Rust with Ratatui.

## Features

- **Pull Requests**: Browse, review, approve, and merge PRs
- **GitHub Actions**: Monitor workflow runs, view jobs, and access logs
- **Full Diff View**: Syntax-highlighted diff viewing with scrolling
- **Log Viewer**: Full-screen log viewer with search functionality
- **Keyboard-Driven**: Vim-style navigation throughout

## Screenshot

```
+-------------------------------------------------------------+
|  [1] PRs    [2] Actions    [3] Logs        shopsys/shopsys  |
+-------------------------------------------------------------+
|                                                              |
|  +- PRs (All) ---------------+ +- PR Details --------------+ |
|  | > #4395 Doctrine annot... | | #4395 Doctrine annotations | |
|  |   #4394 Added upgrade     | | Author: TomasLud662         | |
|  |   #4393 Removed direct... | | Branch: doctrine-attributes | |
|  |   #4392 Dropped support   | | Status: O Open | CI: O      | |
|  |                           | |                             | |
|  |                           | | --- src/Entity/Product.php  | |
|  |                           | | -/** @ORM\Entity */         | |
|  |                           | | +#[ORM\Entity]              | |
|  +---------------------------+ +-----------------------------+ |
|                                                              |
+-------------------------------------------------------------+
| j/k:nav  h/l:panel  Enter:detail  v:approve  d:diff  ?:help |
+-------------------------------------------------------------+
```

## Installation

### Prerequisites

- Rust 1.83+ (install via [rustup](https://rustup.rs/))
- GitHub CLI (`gh`) - for authentication and some operations

### Build from Source

```bash
git clone https://github.com/yourusername/github-tui.git
cd github-tui
cargo build --release
```

The binary will be at `./target/release/github-tui`.

### Authentication

The app uses GitHub authentication in this order:
1. `GITHUB_TOKEN` environment variable
2. `GH_TOKEN` environment variable
3. GitHub CLI token (`gh auth token`)

Easiest setup:
```bash
gh auth login
```

## Usage

```bash
# Auto-detect repo from git remote
github-tui

# Specify repo explicitly
github-tui --repo owner/repo
```

## Key Bindings

### Global

| Key | Action |
|-----|--------|
| `1` | Switch to PRs tab |
| `2` | Switch to Actions tab |
| `3` | Switch to Logs tab |
| `r` | Refresh current view |
| `?` | Toggle help overlay |
| `q` | Quit |
| `Ctrl+C` | Force quit |

### PRs Tab

| Key | Action |
|-----|--------|
| `j/k` | Navigate list / scroll diff |
| `h/l` | Switch between list and detail panels |
| `Enter` | View PR details |
| `d` | View full diff |
| `v` | Approve PR |
| `x` | Request changes |
| `c` | Add comment |
| `m` | Merge PR (squash) |
| `C` | Checkout PR branch |
| `f` | Cycle filter (All/Mine/Review Requested) |
| `Esc` | Back to list |

### Actions Tab

| Key | Action |
|-----|--------|
| `j/k` | Navigate runs/jobs |
| `Enter` | View jobs for selected run |
| `L` | View logs |
| `R` | Rerun workflow |
| `Esc` | Back to runs |

### Logs Tab

| Key | Action |
|-----|--------|
| `j/k` | Scroll up/down |
| `g/G` | Go to top/bottom |
| `PgUp/PgDn` | Page scroll |
| `/` | Search |
| `n/N` | Next/previous match |
| `Esc` | Return to Actions |

## Architecture

```
github-tui/
├── src/
│   ├── main.rs          # Entry point, terminal setup
│   ├── app.rs           # Application state and event handling
│   ├── event.rs         # Async event handler
│   ├── ui/
│   │   ├── render.rs    # Main render function
│   │   ├── styles.rs    # Color themes
│   │   ├── pr_list.rs   # PR list component
│   │   ├── pr_detail.rs # PR detail + diff view
│   │   ├── actions_list.rs
│   │   ├── jobs_view.rs
│   │   ├── log_viewer.rs
│   │   └── help.rs      # Help overlay
│   └── github/
│       ├── client.rs    # GitHub API client
│       └── types.rs     # Data types
└── Cargo.toml
```

## Tech Stack

- **[Ratatui](https://ratatui.rs/)** - Terminal UI framework
- **[Crossterm](https://github.com/crossterm-rs/crossterm)** - Terminal manipulation
- **[Tokio](https://tokio.rs/)** - Async runtime
- **[Octocrab](https://github.com/XAMPPRocky/octocrab)** - GitHub API client
- **GitHub CLI** - For diffs, logs, and some operations

## License

MIT
