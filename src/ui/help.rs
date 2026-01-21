use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

use super::styles;

pub fn render(frame: &mut Frame, _app: &App) {
    let area = centered_rect(60, 30, frame.area());

    let help_text = vec![
        Line::from(Span::styled("Global Keys", styles::TEXT_BOLD)),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Tab      ", styles::TAB_ACTIVE),
            Span::styled("Next tab / Shift+Tab: Previous tab", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  1/2/3    ", styles::TAB_ACTIVE),
            Span::styled("Jump to tab (PRs/Actions/Logs)", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  r        ", styles::TAB_ACTIVE),
            Span::styled("Refresh current view", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  ?        ", styles::TAB_ACTIVE),
            Span::styled("Toggle help", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  q        ", styles::TAB_ACTIVE),
            Span::styled("Quit", styles::TEXT_NORMAL),
        ]),
        Line::from(""),
        Line::from(Span::styled("PRs Tab", styles::TEXT_BOLD)),
        Line::from(""),
        Line::from(vec![
            Span::styled("  j/k      ", styles::TAB_ACTIVE),
            Span::styled("Navigate list / scroll diff", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  h/l      ", styles::TAB_ACTIVE),
            Span::styled("Switch between list and detail panels", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", styles::TAB_ACTIVE),
            Span::styled("View PR details", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  d        ", styles::TAB_ACTIVE),
            Span::styled("View full diff", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  v        ", styles::TAB_ACTIVE),
            Span::styled("Approve PR", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  x        ", styles::TAB_ACTIVE),
            Span::styled("Request changes", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  c        ", styles::TAB_ACTIVE),
            Span::styled("Add comment", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  m        ", styles::TAB_ACTIVE),
            Span::styled("Merge PR (squash)", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  C        ", styles::TAB_ACTIVE),
            Span::styled("Checkout PR branch", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  y        ", styles::TAB_ACTIVE),
            Span::styled("Copy branch name to clipboard", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  Y        ", styles::TAB_ACTIVE),
            Span::styled("Copy checkout command to clipboard", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  u        ", styles::TAB_ACTIVE),
            Span::styled("Copy PR URL to clipboard", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  f        ", styles::TAB_ACTIVE),
            Span::styled("Cycle filter (All/Mine/Review)", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  n        ", styles::TAB_ACTIVE),
            Span::styled("Create new PR (opens browser)", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  o        ", styles::TAB_ACTIVE),
            Span::styled("Cycle focus: List -> Detail -> CI Checks", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  R        ", styles::TAB_ACTIVE),
            Span::styled("Rerun selected CI check (in CI panel)", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  L        ", styles::TAB_ACTIVE),
            Span::styled("View jobs for CI check (in CI panel)", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  e        ", styles::TAB_ACTIVE),
            Span::styled("Edit PR title", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  a        ", styles::TAB_ACTIVE),
            Span::styled("Add reviewer", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  b        ", styles::TAB_ACTIVE),
            Span::styled("Add label", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  w        ", styles::TAB_ACTIVE),
            Span::styled("Open PR in browser (full edit)", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  p        ", styles::TAB_ACTIVE),
            Span::styled("Toggle commit view (full diff / per-commit)", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  [/]      ", styles::TAB_ACTIVE),
            Span::styled("Previous/next commit (in commit view)", styles::TEXT_NORMAL),
        ]),
        Line::from(""),
        Line::from(Span::styled("Actions Tab", styles::TEXT_BOLD)),
        Line::from(""),
        Line::from(vec![
            Span::styled("  j/k      ", styles::TAB_ACTIVE),
            Span::styled("Navigate runs/jobs", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", styles::TAB_ACTIVE),
            Span::styled("View jobs for run", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  L        ", styles::TAB_ACTIVE),
            Span::styled("View logs", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  R        ", styles::TAB_ACTIVE),
            Span::styled("Rerun workflow", styles::TEXT_NORMAL),
        ]),
        Line::from(""),
        Line::from(Span::styled("Logs Tab", styles::TEXT_BOLD)),
        Line::from(""),
        Line::from(vec![
            Span::styled("  j/k      ", styles::TAB_ACTIVE),
            Span::styled("Scroll up/down", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  h/l      ", styles::TAB_ACTIVE),
            Span::styled("Scroll left/right", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  g/G      ", styles::TAB_ACTIVE),
            Span::styled("Go to top/bottom", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  /        ", styles::TAB_ACTIVE),
            Span::styled("Search", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  n/N      ", styles::TAB_ACTIVE),
            Span::styled("Next/previous match", styles::TEXT_NORMAL),
        ]),
        Line::from(vec![
            Span::styled("  Esc      ", styles::TAB_ACTIVE),
            Span::styled("Return to Actions", styles::TEXT_NORMAL),
        ]),
        Line::from(""),
        Line::from(Span::styled("Press ? or Esc to close", styles::TEXT_DIM)),
    ];

    let help = Paragraph::new(help_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(styles::BORDER_ACTIVE)
            .title(" Help "),
    );

    frame.render_widget(Clear, area);
    frame.render_widget(help, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
