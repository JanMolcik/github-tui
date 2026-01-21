use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use crate::app::App;

use super::styles;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .runs
        .iter()
        .map(|run| {
            let status_style = styles::status_style(&run.status, run.conclusion.as_deref());

            let conclusion_text = run.conclusion.as_deref().unwrap_or(&run.status);

            let line = Line::from(vec![
                Span::styled(run.status_icon(), status_style),
                Span::raw(" "),
                Span::styled(
                    truncate(&run.name, 30),
                    styles::TEXT_NORMAL,
                ),
                Span::raw(" "),
                Span::styled(format!("#{}", run.run_number), styles::TEXT_DIM),
                Span::raw(" "),
                Span::styled(&run.head_branch, styles::TEXT_DIM),
                Span::raw(" "),
                Span::styled(conclusion_text, status_style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(styles::BORDER_ACTIVE)
                .title(" Workflow Runs [R:rerun] "),
        )
        .highlight_style(styles::SELECTED);

    frame.render_stateful_widget(list, area, &mut app.run_list_state.clone());
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
