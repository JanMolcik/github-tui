use ratatui::{
    layout::Rect,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

use super::styles;

/// Strip ANSI escape codes from a string
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip escape sequence
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Skip until we hit a letter (end of escape sequence)
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if c == '\t' {
            // Replace tabs with spaces
            result.push_str("    ");
        } else if c.is_ascii_control() && c != '\n' {
            // Skip other control characters
        } else {
            result.push(c);
        }
    }

    result
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let title = if let Some(ref run) = app.selected_run {
        let job_name = app
            .job_list_state
            .selected()
            .and_then(|i| app.jobs.get(i))
            .map(|j| format!(" - {}", j.name))
            .unwrap_or_default();

        format!(" Logs: {} #{}{} ", run.name, run.run_number, job_name)
    } else {
        " Logs ".to_string()
    };

    if app.logs.is_empty() {
        let placeholder = Paragraph::new("No logs available. Select a job and press Enter or L to view logs.")
            .style(styles::TEXT_DIM)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(styles::BORDER_ACTIVE)
                    .title(title),
            );

        frame.render_widget(placeholder, area);
        return;
    }

    let height = area.height as usize - 2;
    let width = area.width as usize - 2;
    let search_term = app.log_search.as_deref();

    let lines: Vec<Line> = app
        .logs
        .lines()
        .enumerate()
        .skip(app.log_scroll as usize)
        .take(height)
        .map(|(line_num, line)| {
            // Strip ANSI codes and clean the line
            let clean_line = strip_ansi(line);

            // Truncate to terminal width (with horizontal scroll offset)
            let display_line: String = if clean_line.len() > width {
                let start = app.log_h_scroll as usize;
                if start < clean_line.len() {
                    clean_line.chars().skip(start).take(width).collect()
                } else {
                    String::new()
                }
            } else {
                let start = app.log_h_scroll as usize;
                if start < clean_line.len() {
                    clean_line.chars().skip(start).collect()
                } else {
                    String::new()
                }
            };

            // Check if this line is a match
            let is_match = app.log_matches.contains(&line_num);

            // Determine style based on content
            let style = if clean_line.contains("##[group]") || clean_line.contains("##[endgroup]") {
                styles::DIFF_HEADER
            } else if clean_line.contains("##[error]") || clean_line.to_lowercase().contains("error") {
                styles::FAILURE
            } else if clean_line.contains("##[warning]") || clean_line.to_lowercase().contains("warning") {
                styles::PENDING
            } else if is_match {
                styles::HIGHLIGHT
            } else if clean_line.starts_with("Run ") || clean_line.contains("\t") {
                styles::TEXT_DIM
            } else {
                styles::TEXT_NORMAL
            };

            // Highlight search matches
            if let Some(term) = search_term {
                if !term.is_empty() && clean_line.to_lowercase().contains(&term.to_lowercase()) {
                    Line::from(Span::styled(display_line, styles::HIGHLIGHT))
                } else {
                    Line::from(Span::styled(display_line, style))
                }
            } else {
                Line::from(Span::styled(display_line, style))
            }
        })
        .collect();

    let text = Text::from(lines);

    // Build status line
    let total_lines = app.logs.lines().count();
    let current_line = app.log_scroll as usize + 1;
    let percentage = if total_lines > 0 {
        (current_line * 100) / total_lines
    } else {
        0
    };

    let status = if let Some(ref search) = app.log_search {
        format!(
            " Line {}/{} ({}%) | Search: '{}' ({}/{}) | h/l:scroll ",
            current_line,
            total_lines,
            percentage,
            search,
            if app.log_matches.is_empty() {
                0
            } else {
                app.log_match_index + 1
            },
            app.log_matches.len()
        )
    } else {
        format!(" Line {}/{} ({}%) | h/l:horizontal scroll ", current_line, total_lines, percentage)
    };

    let log_widget = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(styles::BORDER_ACTIVE)
                .title(title)
                .title_bottom(status),
        );

    frame.render_widget(log_widget, area);
}
