use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs},
    Frame,
};

use crate::app::{App, InputMode, Tab, View};

use super::{actions_list, help, jobs_view, log_viewer, pr_detail, pr_list, styles};

pub fn render(frame: &mut Frame, app: &mut App) {
    // Main layout: header, content, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header with tabs
            Constraint::Min(0),    // Content
            Constraint::Length(2), // Footer with help
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);
    render_content(frame, app, chunks[1]);
    render_footer(frame, app, chunks[2]);

    // Render overlays
    if app.show_help {
        help::render(frame, app);
    }

    if app.input_mode.is_some() {
        render_input(frame, app);
    }

    // Render loading overlay
    if app.loading {
        render_loading(frame);
    }
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(30), Constraint::Length(30)])
        .split(area);

    // Tabs
    let tab_titles = vec!["[1] PRs", "[2] Actions", "[3] Logs"];
    let selected = match app.tab {
        Tab::PRs => 0,
        Tab::Actions => 1,
        Tab::Logs => 2,
    };

    let tabs = Tabs::new(tab_titles)
        .select(selected)
        .style(styles::TAB_INACTIVE)
        .highlight_style(styles::TAB_ACTIVE)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(styles::BORDER_ACTIVE)
                .title(" GitHub TUI "),
        );

    frame.render_widget(tabs, header_chunks[0]);

    // Repo info
    let repo_info = Paragraph::new(app.repo.clone())
        .style(styles::TEXT_DIM)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(styles::BORDER_INACTIVE),
        );

    frame.render_widget(repo_info, header_chunks[1]);
}

fn render_content(frame: &mut Frame, app: &mut App, area: Rect) {
    match app.tab {
        Tab::PRs => render_pr_content(frame, app, area),
        Tab::Actions => render_actions_content(frame, app, area),
        Tab::Logs => log_viewer::render(frame, app, area),
    }
}

fn render_pr_content(frame: &mut Frame, app: &mut App, area: Rect) {
    match app.view {
        View::Diff => {
            pr_detail::render_full_diff(frame, app, area);
        }
        _ => {
            // Split into list and detail
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(area);

            pr_list::render(frame, app, chunks[0]);
            pr_detail::render(frame, app, chunks[1]);
        }
    }
}

fn render_actions_content(frame: &mut Frame, app: &App, area: Rect) {
    match app.view {
        View::Jobs => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(area);

            actions_list::render(frame, app, chunks[0]);
            jobs_view::render(frame, app, chunks[1]);
        }
        _ => {
            actions_list::render(frame, app, area);
        }
    }
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    // Error or message display
    let status_line = if let Some(ref err) = app.error {
        Line::from(vec![
            Span::styled("Error: ", styles::ERROR),
            Span::styled(err.as_str(), styles::ERROR),
        ])
    } else if let Some(ref msg) = app.message {
        Line::from(Span::styled(msg.as_str(), styles::MESSAGE))
    } else {
        // Context-sensitive help
        let help_text = match app.tab {
            Tab::PRs => match app.view {
                View::Diff => "j/k:scroll  Esc:back  ?:help  q:quit",
                _ => "j/k:nav  h/l:panel  Enter:detail  v:approve  d:diff  r:refresh  ?:help  q:quit",
            },
            Tab::Actions => match app.view {
                View::Jobs => "j/k:nav  Enter/L:logs  R:rerun  Esc:back  ?:help  q:quit",
                _ => "j/k:nav  Enter:jobs  R:rerun  r:refresh  ?:help  q:quit",
            },
            Tab::Logs => "j/k:scroll  /:search  n:next  N:prev  Esc:back  ?:help  q:quit",
        };
        Line::from(Span::styled(help_text, styles::TEXT_DIM))
    };

    let footer = Paragraph::new(status_line).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(styles::BORDER_INACTIVE),
    );

    frame.render_widget(footer, area);
}

fn render_input(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 3, frame.area());

    let title = match app.input_mode {
        Some(InputMode::Search) => " Search ",
        Some(InputMode::Comment) => " Comment ",
        None => "",
    };

    let input = Paragraph::new(app.input_buffer.as_str())
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(styles::BORDER_ACTIVE)
                .title(title),
        );

    frame.render_widget(Clear, area);
    frame.render_widget(input, area);

    // Show cursor
    frame.set_cursor_position((area.x + app.input_buffer.len() as u16 + 1, area.y + 1));
}

fn render_loading(frame: &mut Frame) {
    let area = centered_rect(20, 3, frame.area());

    let loading = Paragraph::new("Loading...")
        .style(styles::LOADING)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(styles::BORDER_ACTIVE),
        );

    frame.render_widget(Clear, area);
    frame.render_widget(loading, area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((area.width.saturating_sub(width)) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(popup_layout[1])[1]
}
