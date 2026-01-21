use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use crate::app::{App, Focus, PrFilter};

use super::styles;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == Focus::List;

    let filter_text = match app.pr_filter {
        PrFilter::All => "All",
        PrFilter::Mine => "Mine",
        PrFilter::ReviewRequested => "Review Requested",
    };

    let title = format!(" PRs ({}) [f:filter] ", filter_text);

    let items: Vec<ListItem> = app
        .prs
        .iter()
        .map(|pr| {
            let style = styles::pr_style(&pr.state, pr.merged, pr.draft);

            let line = Line::from(vec![
                Span::styled(pr.status_icon(), style),
                Span::raw(" "),
                Span::styled(format!("#{}", pr.number), styles::TEXT_BOLD),
                Span::raw(" "),
                Span::styled(
                    truncate(&pr.title, (area.width as usize).saturating_sub(20)),
                    styles::TEXT_NORMAL,
                ),
                Span::raw(" "),
                Span::styled(format!("@{}", pr.user.login), styles::TEXT_DIM),
            ]);

            ListItem::new(line)
        })
        .collect();

    let border_style = if is_focused {
        styles::BORDER_ACTIVE
    } else {
        styles::BORDER_INACTIVE
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .highlight_style(styles::SELECTED);

    frame.render_stateful_widget(list, area, &mut app.pr_list_state.clone());
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}
