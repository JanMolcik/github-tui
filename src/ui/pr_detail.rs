use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, DiffMode, Focus};

use super::styles;

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let detail_focused = app.focus == Focus::Detail;
    let checks_focused = app.focus == Focus::PrChecks;

    let detail_border = if detail_focused {
        styles::BORDER_ACTIVE
    } else {
        styles::BORDER_INACTIVE
    };

    let checks_border = if checks_focused {
        styles::BORDER_ACTIVE
    } else {
        styles::BORDER_INACTIVE
    };

    if let Some(ref pr) = app.selected_pr {
        // Split into metadata, diff preview, and checks panel
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),  // Metadata
                Constraint::Min(10),    // Diff preview
                Constraint::Length(10), // PR Checks
            ])
            .split(area);

        // Metadata section
        let status_style = styles::pr_style(&pr.state, pr.merged, pr.draft);
        let ci_style = match pr.ci_status.as_deref() {
            Some("success") => styles::SUCCESS,
            Some("failure") => styles::FAILURE,
            Some("pending") => styles::PENDING,
            _ => styles::NEUTRAL,
        };

        let labels_text = if pr.labels.is_empty() {
            String::new()
        } else {
            pr.labels.iter().map(|l| l.name.as_str()).collect::<Vec<_>>().join(", ")
        };

        let reviewers_text = if pr.requested_reviewers.is_empty() {
            "None".to_string()
        } else {
            pr.requested_reviewers
                .iter()
                .map(|r| r.login.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };

        let meta_lines = vec![
            Line::from(vec![
                Span::styled(format!("#{} ", pr.number), styles::TEXT_BOLD),
                Span::styled(&pr.title, styles::TEXT_NORMAL),
            ]),
            Line::from(vec![
                Span::styled("Author: ", styles::TEXT_DIM),
                Span::styled(&pr.user.login, styles::TEXT_NORMAL),
            ]),
            Line::from(vec![
                Span::styled("Branch: ", styles::TEXT_DIM),
                Span::styled(&pr.head.ref_name, styles::TEXT_NORMAL),
                Span::styled(" -> ", styles::TEXT_DIM),
                Span::styled(&pr.base.ref_name, styles::TEXT_NORMAL),
            ]),
            Line::from(vec![
                Span::styled("Status: ", styles::TEXT_DIM),
                Span::styled(pr.status_icon(), status_style),
                Span::styled(
                    if pr.merged {
                        " Merged"
                    } else if pr.state == "closed" {
                        " Closed"
                    } else if pr.draft {
                        " Draft"
                    } else {
                        " Open"
                    },
                    status_style,
                ),
                Span::styled(" | CI: ", styles::TEXT_DIM),
                Span::styled(pr.ci_icon(), ci_style),
            ]),
            Line::from(vec![
                Span::styled("Reviewers: ", styles::TEXT_DIM),
                Span::styled(reviewers_text, styles::TEXT_NORMAL),
            ]),
            Line::from(vec![
                Span::styled("Labels: ", styles::TEXT_DIM),
                Span::styled(labels_text, styles::TEXT_NORMAL),
            ]),
        ];

        let meta = Paragraph::new(meta_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(detail_border)
                .title(" PR Details [d:full diff] "),
        );

        frame.render_widget(meta, chunks[0]);

        // Diff area - changes based on mode
        match app.diff_mode {
            DiffMode::Full => {
                // Full diff preview
                if let Some(ref diff) = app.pr_diff {
                    let diff_lines = render_diff_lines(diff, app.diff_scroll as usize, chunks[1].height as usize - 2);

                    let diff_widget = Paragraph::new(diff_lines)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(detail_border)
                                .title(" Diff Preview [p:commits, j/k:scroll] "),
                        )
                        .wrap(Wrap { trim: false });

                    frame.render_widget(diff_widget, chunks[1]);
                } else {
                    let placeholder = Paragraph::new("Loading diff...")
                        .style(styles::TEXT_DIM)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(detail_border)
                                .title(" Diff Preview [p:commits] "),
                        );

                    frame.render_widget(placeholder, chunks[1]);
                }
            }
            DiffMode::ByCommit => {
                // Split into commit list and commit diff
                let commit_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Length(40), Constraint::Min(20)])
                    .split(chunks[1]);

                // Commit list
                render_commit_list(frame, app, commit_chunks[0], detail_border);

                // Commit diff
                if let Some(ref diff) = app.commit_diff {
                    let diff_lines = render_diff_lines(diff, app.diff_scroll as usize, commit_chunks[1].height as usize - 2);

                    let commit_info = app.pr_commits_state.selected()
                        .and_then(|i| app.pr_commits.get(i))
                        .map(|c| format!(" {} ", c.short_sha()))
                        .unwrap_or_else(|| " Commit ".to_string());

                    let diff_widget = Paragraph::new(diff_lines)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(detail_border)
                                .title(format!(" Commit {} [j/k:scroll, p:full diff] ", commit_info)),
                        )
                        .wrap(Wrap { trim: false });

                    frame.render_widget(diff_widget, commit_chunks[1]);
                } else {
                    let placeholder = Paragraph::new("Select a commit to view diff...")
                        .style(styles::TEXT_DIM)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(detail_border)
                                .title(" Commit Diff [p:full diff] "),
                        );

                    frame.render_widget(placeholder, commit_chunks[1]);
                }
            }
        }

        // PR Checks panel
        render_pr_checks(frame, app, chunks[2], checks_border);
    } else {
        let placeholder = Paragraph::new("Select a PR to view details")
            .style(styles::TEXT_DIM)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(detail_border)
                    .title(" PR Details "),
            );

        frame.render_widget(placeholder, area);
    }
}

fn render_commit_list(frame: &mut Frame, app: &mut App, area: Rect, border_style: ratatui::style::Style) {
    let commit_count = app.pr_commits.len();
    let selected_idx = app.pr_commits_state.selected().unwrap_or(0);

    let items: Vec<ListItem> = app
        .pr_commits
        .iter()
        .enumerate()
        .map(|(i, commit)| {
            let marker = if i == selected_idx { ">" } else { " " };
            let line = Line::from(vec![
                Span::styled(marker, styles::TEXT_BOLD),
                Span::styled(format!(" {} ", commit.short_sha()), styles::DIFF_HEADER),
                Span::styled(commit.first_line(), styles::TEXT_NORMAL),
            ]);
            ListItem::new(line)
        })
        .collect();

    let title = format!(" Commits ({}/{}) [/]:nav ", selected_idx + 1, commit_count);

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .highlight_style(styles::HIGHLIGHT)
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut app.pr_commits_state);
}

fn render_pr_checks(frame: &mut Frame, app: &mut App, area: Rect, border_style: ratatui::style::Style) {
    if app.pr_checks.is_empty() {
        let placeholder = Paragraph::new("No workflow runs found for this PR")
            .style(styles::TEXT_DIM)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(" CI Checks [Tab:focus, R:rerun, L:logs] "),
            );
        frame.render_widget(placeholder, area);
        return;
    }

    let items: Vec<ListItem> = app
        .pr_checks
        .iter()
        .map(|run| {
            let status_style = match run.conclusion.as_deref() {
                Some("success") => styles::SUCCESS,
                Some("failure") => styles::FAILURE,
                Some("cancelled") | Some("skipped") => styles::NEUTRAL,
                _ => match run.status.as_str() {
                    "in_progress" | "queued" => styles::PENDING,
                    _ => styles::TEXT_NORMAL,
                },
            };

            let status_text = run.conclusion.as_deref()
                .unwrap_or(&run.status);

            let line = Line::from(vec![
                Span::styled(run.status_icon(), status_style),
                Span::raw(" "),
                Span::styled(&run.name, styles::TEXT_NORMAL),
                Span::styled(" (", styles::TEXT_DIM),
                Span::styled(status_text, status_style),
                Span::styled(")", styles::TEXT_DIM),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" CI Checks [Tab:focus, R:rerun, L:logs] "),
        )
        .highlight_style(styles::HIGHLIGHT)
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut app.pr_checks_state);
}

pub fn render_full_diff(frame: &mut Frame, app: &App, area: Rect) {
    if let Some(ref diff) = app.pr_diff {
        let diff_lines = render_diff_lines(diff, app.diff_scroll as usize, area.height as usize - 2);

        let pr_title = app
            .selected_pr
            .as_ref()
            .map(|pr| format!(" #{} - {} ", pr.number, pr.title))
            .unwrap_or_else(|| " Diff ".to_string());

        let diff_widget = Paragraph::new(diff_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(styles::BORDER_ACTIVE)
                    .title(pr_title),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(diff_widget, area);
    }
}

fn render_diff_lines(diff: &str, scroll: usize, height: usize) -> Text<'static> {
    // First, process the diff to add file separators
    let mut processed_lines: Vec<Line> = Vec::new();
    let mut current_file: Option<String> = None;

    for line in diff.lines() {
        // Detect new file from "diff --git a/path b/path" line
        if line.starts_with("diff --git ") {
            // Extract filename from the line
            if let Some(filename) = extract_filename_from_diff_line(line) {
                // Add separator if not the first file
                if current_file.is_some() {
                    processed_lines.push(Line::from(""));
                }

                // Create a prominent file header
                let separator = "─".repeat(60);
                processed_lines.push(Line::from(Span::styled(
                    separator.clone(),
                    styles::DIFF_HEADER,
                )));
                processed_lines.push(Line::from(vec![
                    Span::styled(">> ", styles::DIFF_HEADER),
                    Span::styled(filename.clone(), styles::TEXT_BOLD),
                ]));
                processed_lines.push(Line::from(Span::styled(
                    separator,
                    styles::DIFF_HEADER,
                )));

                current_file = Some(filename);
            }
            continue; // Skip the original diff --git line
        }

        // Skip index lines (less useful noise)
        if line.starts_with("index ") {
            continue;
        }

        // Skip the +++ and --- file path lines (we already show the filename)
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }

        // Style the remaining lines
        let style = if line.starts_with('+') {
            styles::DIFF_ADD
        } else if line.starts_with('-') {
            styles::DIFF_REMOVE
        } else if line.starts_with("@@") {
            styles::DIFF_HUNK
        } else {
            styles::TEXT_NORMAL
        };

        processed_lines.push(Line::from(Span::styled(line.to_string(), style)));
    }

    // Apply scroll and height limits
    let visible_lines: Vec<Line> = processed_lines
        .into_iter()
        .skip(scroll)
        .take(height)
        .collect();

    Text::from(visible_lines)
}

fn extract_filename_from_diff_line(line: &str) -> Option<String> {
    // Format: "diff --git a/path/to/file b/path/to/file"
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 4 {
        // Get the b/path part and remove the "b/" prefix
        let b_path = parts[3];
        if b_path.starts_with("b/") {
            return Some(b_path[2..].to_string());
        }
        return Some(b_path.to_string());
    }
    None
}
