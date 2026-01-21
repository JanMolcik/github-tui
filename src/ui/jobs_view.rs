use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use crate::app::App;

use super::styles;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let run_title = app
        .selected_run
        .as_ref()
        .map(|r| format!(" Jobs - {} #{} ", r.name, r.run_number))
        .unwrap_or_else(|| " Jobs ".to_string());

    if app.jobs.is_empty() {
        let placeholder = ratatui::widgets::Paragraph::new("Select a run to view jobs")
            .style(styles::TEXT_DIM)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(styles::BORDER_ACTIVE)
                    .title(run_title),
            );

        frame.render_widget(placeholder, area);
        return;
    }

    let items: Vec<ListItem> = app
        .jobs
        .iter()
        .map(|job| {
            let status_style = styles::status_style(&job.status, job.conclusion.as_deref());

            let conclusion_text = job.conclusion.as_deref().unwrap_or(&job.status);

            let line = Line::from(vec![
                Span::styled(job.status_icon(), status_style),
                Span::raw(" "),
                Span::styled(&job.name, styles::TEXT_NORMAL),
                Span::raw(" "),
                Span::styled(format!("[{}]", conclusion_text), status_style),
                Span::raw(" "),
                Span::styled(job.duration(), styles::TEXT_DIM),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(styles::BORDER_ACTIVE)
                .title(format!("{} [Enter/L:logs] ", run_title)),
        )
        .highlight_style(styles::SELECTED);

    frame.render_stateful_widget(list, area, &mut app.job_list_state.clone());
}
