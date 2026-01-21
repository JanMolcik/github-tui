use ratatui::style::{Color, Modifier, Style};

// Tab colors
pub const TAB_ACTIVE: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
pub const TAB_INACTIVE: Style = Style::new().fg(Color::DarkGray);

// Status colors
pub const SUCCESS: Style = Style::new().fg(Color::Green);
pub const FAILURE: Style = Style::new().fg(Color::Red);
pub const PENDING: Style = Style::new().fg(Color::Yellow);
pub const NEUTRAL: Style = Style::new().fg(Color::DarkGray);

// PR state colors
pub const PR_OPEN: Style = Style::new().fg(Color::Green);
pub const PR_CLOSED: Style = Style::new().fg(Color::Red);
pub const PR_MERGED: Style = Style::new().fg(Color::Magenta);
pub const PR_DRAFT: Style = Style::new().fg(Color::DarkGray);

// Diff colors
pub const DIFF_ADD: Style = Style::new().fg(Color::Green);
pub const DIFF_REMOVE: Style = Style::new().fg(Color::Red);
pub const DIFF_HEADER: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
pub const DIFF_HUNK: Style = Style::new().fg(Color::Blue);

// Selection
pub const SELECTED: Style = Style::new()
    .bg(Color::DarkGray)
    .add_modifier(Modifier::BOLD);
pub const HIGHLIGHT: Style = Style::new().bg(Color::Yellow).fg(Color::Black);

// Borders
pub const BORDER_ACTIVE: Style = Style::new()
    .fg(Color::LightCyan)
    .add_modifier(Modifier::BOLD);
pub const BORDER_INACTIVE: Style = Style::new().fg(Color::Rgb(60, 60, 60));

// Text
pub const TEXT_NORMAL: Style = Style::new().fg(Color::White);
pub const TEXT_DIM: Style = Style::new().fg(Color::DarkGray);
pub const TEXT_BOLD: Style = Style::new()
    .fg(Color::White)
    .add_modifier(Modifier::BOLD);

// Error/Message
pub const ERROR: Style = Style::new().fg(Color::Red).add_modifier(Modifier::BOLD);
pub const MESSAGE: Style = Style::new().fg(Color::Green);

// Loading
pub const LOADING: Style = Style::new().fg(Color::Yellow);

// Helper to get status style
pub fn status_style(status: &str, conclusion: Option<&str>) -> Style {
    match conclusion {
        Some("success") => SUCCESS,
        Some("failure") => FAILURE,
        Some("cancelled") | Some("skipped") => NEUTRAL,
        _ => match status {
            "in_progress" | "queued" => PENDING,
            _ => NEUTRAL,
        },
    }
}

// Helper to get PR style
pub fn pr_style(state: &str, merged: bool, draft: bool) -> Style {
    if merged {
        PR_MERGED
    } else if state == "closed" {
        PR_CLOSED
    } else if draft {
        PR_DRAFT
    } else {
        PR_OPEN
    }
}
