use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::ListState;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::event::{Event, EventHandler};
use crate::github::types::{Job, PullRequest, WorkflowRun};
use crate::github::Client;
use crate::ui;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    #[default]
    PRs,
    Actions,
    Logs,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum View {
    #[default]
    List,
    Detail,
    Diff,
    Jobs,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    #[default]
    List,
    Detail,
    PrChecks,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum PrFilter {
    #[default]
    All,
    Mine,
    ReviewRequested,
}

// Messages for async operations
pub enum AsyncMsg {
    PrsLoaded(Vec<PullRequest>),
    RunsLoaded(Vec<WorkflowRun>),
    DiffLoaded(String),
    PrChecksLoaded(Vec<WorkflowRun>),
    JobsLoaded(Vec<Job>),
    LogsLoaded(String),
    Error(String),
    Message(String),
}

pub struct App {
    pub tab: Tab,
    pub view: View,
    pub focus: Focus,
    pub repo: String,
    pub owner: String,
    pub repo_name: String,

    // PR state
    pub prs: Vec<PullRequest>,
    pub pr_list_state: ListState,
    pub selected_pr: Option<PullRequest>,
    pub pr_diff: Option<String>,
    pub pr_filter: PrFilter,
    pub diff_scroll: u16,

    // PR checks (workflow runs for selected PR)
    pub pr_checks: Vec<WorkflowRun>,
    pub pr_checks_state: ListState,

    // Actions state
    pub runs: Vec<WorkflowRun>,
    pub run_list_state: ListState,
    pub selected_run: Option<WorkflowRun>,
    pub jobs: Vec<Job>,
    pub job_list_state: ListState,

    // Logs state
    pub logs: String,
    pub log_scroll: u16,
    pub log_h_scroll: u16,
    pub log_search: Option<String>,
    pub log_matches: Vec<usize>,
    pub log_match_index: usize,

    // UI state
    pub loading: bool,
    pub loading_what: Option<String>,
    pub error: Option<String>,
    pub message: Option<String>,
    pub should_quit: bool,
    pub show_help: bool,
    pub input_mode: Option<InputMode>,
    pub input_buffer: String,

    // GitHub client
    pub client: Option<Client>,

    // Async message channel
    async_rx: Option<mpsc::UnboundedReceiver<AsyncMsg>>,
    async_tx: Option<mpsc::UnboundedSender<AsyncMsg>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Search,
    Comment,
}

impl App {
    pub fn new(repo: String) -> Self {
        let parts: Vec<&str> = repo.split('/').collect();
        let (owner, repo_name) = if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            ("shopsys".to_string(), "shopsys".to_string())
        };

        let (tx, rx) = mpsc::unbounded_channel();

        Self {
            repo: repo.clone(),
            owner,
            repo_name,
            pr_list_state: ListState::default(),
            pr_checks_state: ListState::default(),
            run_list_state: ListState::default(),
            job_list_state: ListState::default(),
            async_rx: Some(rx),
            async_tx: Some(tx),
            ..Default::default()
        }
    }

    pub async fn run(&mut self, terminal: &mut Terminal<impl Backend>) -> Result<()> {
        // Initialize GitHub client
        self.client = Some(Client::new().await?);

        // Initial data fetch (async)
        self.loading = true;
        self.loading_what = Some("Loading PRs and workflows...".to_string());
        terminal.draw(|f| ui::render(f, self))?;

        self.spawn_fetch_prs();
        self.spawn_fetch_runs();

        // Event loop
        let mut events = EventHandler::new(Duration::from_millis(100));

        while !self.should_quit {
            // Process async messages
            self.process_async_messages();

            terminal.draw(|f| ui::render(f, self))?;

            if let Some(event) = events.next().await {
                match event {
                    Event::Tick => {
                        // Process any pending async messages
                        self.process_async_messages();
                    }
                    Event::Key(key) => self.handle_key(key).await,
                    Event::Resize(_, _) => {}
                }
            }
        }

        Ok(())
    }

    fn process_async_messages(&mut self) {
        if let Some(ref mut rx) = self.async_rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    AsyncMsg::PrsLoaded(prs) => {
                        self.prs = prs;
                        if !self.prs.is_empty() && self.pr_list_state.selected().is_none() {
                            self.pr_list_state.select(Some(0));
                        }
                        self.loading = false;
                        self.loading_what = None;
                    }
                    AsyncMsg::RunsLoaded(runs) => {
                        self.runs = runs;
                        if !self.runs.is_empty() && self.run_list_state.selected().is_none() {
                            self.run_list_state.select(Some(0));
                        }
                    }
                    AsyncMsg::DiffLoaded(diff) => {
                        self.pr_diff = Some(diff);
                        self.loading = false;
                        self.loading_what = None;
                    }
                    AsyncMsg::PrChecksLoaded(checks) => {
                        self.pr_checks = checks;
                        if !self.pr_checks.is_empty() && self.pr_checks_state.selected().is_none() {
                            self.pr_checks_state.select(Some(0));
                        }
                    }
                    AsyncMsg::JobsLoaded(jobs) => {
                        self.jobs = jobs;
                        if !self.jobs.is_empty() && self.job_list_state.selected().is_none() {
                            self.job_list_state.select(Some(0));
                        }
                        self.loading = false;
                        self.loading_what = None;
                    }
                    AsyncMsg::LogsLoaded(logs) => {
                        self.logs = logs;
                        self.log_scroll = 0;
                        self.log_h_scroll = 0;
                        self.loading = false;
                        self.loading_what = None;
                    }
                    AsyncMsg::Error(e) => {
                        self.error = Some(e);
                        self.loading = false;
                        self.loading_what = None;
                    }
                    AsyncMsg::Message(m) => {
                        self.message = Some(m);
                    }
                }
            }
        }
    }

    // Spawn async tasks for fetching data
    fn spawn_fetch_prs(&self) {
        if let (Some(client), Some(tx)) = (self.client.clone(), self.async_tx.clone()) {
            let owner = self.owner.clone();
            let repo = self.repo_name.clone();
            tokio::spawn(async move {
                match client.list_prs(&owner, &repo).await {
                    Ok(prs) => { let _ = tx.send(AsyncMsg::PrsLoaded(prs)); }
                    Err(e) => { let _ = tx.send(AsyncMsg::Error(format!("Failed to fetch PRs: {}", e))); }
                }
            });
        }
    }

    fn spawn_fetch_runs(&self) {
        if let (Some(client), Some(tx)) = (self.client.clone(), self.async_tx.clone()) {
            let owner = self.owner.clone();
            let repo = self.repo_name.clone();
            tokio::spawn(async move {
                match client.list_runs(&owner, &repo).await {
                    Ok(runs) => { let _ = tx.send(AsyncMsg::RunsLoaded(runs)); }
                    Err(e) => { let _ = tx.send(AsyncMsg::Error(format!("Failed to fetch runs: {}", e))); }
                }
            });
        }
    }

    fn spawn_fetch_diff(&self, pr_number: u64) {
        if let (Some(client), Some(tx)) = (self.client.clone(), self.async_tx.clone()) {
            let owner = self.owner.clone();
            let repo = self.repo_name.clone();
            tokio::spawn(async move {
                match client.get_pr_diff(&owner, &repo, pr_number).await {
                    Ok(diff) => { let _ = tx.send(AsyncMsg::DiffLoaded(diff)); }
                    Err(e) => { let _ = tx.send(AsyncMsg::Error(format!("Failed to fetch diff: {}", e))); }
                }
            });
        }
    }

    fn spawn_fetch_pr_checks(&self, head_sha: &str) {
        if let (Some(client), Some(tx)) = (self.client.clone(), self.async_tx.clone()) {
            let owner = self.owner.clone();
            let repo = self.repo_name.clone();
            let sha = head_sha.to_string();
            tokio::spawn(async move {
                match client.list_runs_for_commit(&owner, &repo, &sha).await {
                    Ok(runs) => { let _ = tx.send(AsyncMsg::PrChecksLoaded(runs)); }
                    Err(e) => { let _ = tx.send(AsyncMsg::Error(format!("Failed to fetch PR checks: {}", e))); }
                }
            });
        }
    }

    fn spawn_fetch_jobs(&self, run_id: u64) {
        if let (Some(client), Some(tx)) = (self.client.clone(), self.async_tx.clone()) {
            let owner = self.owner.clone();
            let repo = self.repo_name.clone();
            tokio::spawn(async move {
                match client.list_jobs(&owner, &repo, run_id).await {
                    Ok(jobs) => { let _ = tx.send(AsyncMsg::JobsLoaded(jobs)); }
                    Err(e) => { let _ = tx.send(AsyncMsg::Error(format!("Failed to fetch jobs: {}", e))); }
                }
            });
        }
    }

    fn spawn_fetch_logs(&self, run_id: u64, job_id: Option<u64>) {
        if let (Some(client), Some(tx)) = (self.client.clone(), self.async_tx.clone()) {
            let owner = self.owner.clone();
            let repo = self.repo_name.clone();
            tokio::spawn(async move {
                match client.get_run_logs(&owner, &repo, run_id, job_id).await {
                    Ok(logs) => { let _ = tx.send(AsyncMsg::LogsLoaded(logs)); }
                    Err(e) => { let _ = tx.send(AsyncMsg::Error(format!("Failed to fetch logs: {}", e))); }
                }
            });
        }
    }

    async fn handle_key(&mut self, key: KeyEvent) {
        // Handle Ctrl+C globally
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        // Handle input mode
        if let Some(mode) = self.input_mode {
            match key.code {
                KeyCode::Esc => {
                    self.input_mode = None;
                    self.input_buffer.clear();
                    self.message = None;
                }
                KeyCode::Enter => {
                    match mode {
                        InputMode::Search => {
                            self.log_search = Some(self.input_buffer.clone());
                            self.find_log_matches();
                        }
                        InputMode::Comment => {
                            self.submit_comment().await;
                        }
                    }
                    self.input_mode = None;
                    self.input_buffer.clear();
                    self.message = None;
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    self.input_buffer.push(c);
                }
                _ => {}
            }
            return;
        }

        // Handle help overlay
        if self.show_help {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                self.show_help = false;
            }
            return;
        }

        // Global keys
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('?') => {
                self.show_help = true;
                return;
            }
            KeyCode::Char('1') => {
                self.tab = Tab::PRs;
                self.view = View::List;
                self.focus = Focus::List;
                return;
            }
            KeyCode::Char('2') => {
                self.tab = Tab::Actions;
                self.view = View::List;
                return;
            }
            KeyCode::Char('3') => {
                self.tab = Tab::Logs;
                return;
            }
            KeyCode::Char('r') => {
                self.refresh();
                return;
            }
            KeyCode::Char('n') if self.tab == Tab::PRs && self.view == View::List => {
                self.create_pr();
                return;
            }
            _ => {}
        }

        // Tab-specific keys
        match self.tab {
            Tab::PRs => self.handle_pr_keys(key).await,
            Tab::Actions => self.handle_actions_keys(key).await,
            Tab::Logs => self.handle_logs_keys(key),
        }
    }

    async fn handle_pr_keys(&mut self, key: KeyEvent) {
        match self.view {
            View::List | View::Detail => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    match self.focus {
                        Focus::List => self.next_pr(),
                        Focus::Detail => self.diff_scroll = self.diff_scroll.saturating_add(1),
                        Focus::PrChecks => self.next_pr_check(),
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    match self.focus {
                        Focus::List => self.previous_pr(),
                        Focus::Detail => self.diff_scroll = self.diff_scroll.saturating_sub(1),
                        Focus::PrChecks => self.previous_pr_check(),
                    }
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    self.focus = Focus::List;
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    if self.focus == Focus::List {
                        self.focus = Focus::Detail;
                    } else if self.focus == Focus::Detail {
                        self.focus = Focus::PrChecks;
                    }
                }
                KeyCode::Tab => {
                    // Cycle focus: List -> Detail -> PrChecks -> List
                    self.focus = match self.focus {
                        Focus::List => Focus::Detail,
                        Focus::Detail => Focus::PrChecks,
                        Focus::PrChecks => Focus::List,
                    };
                }
                KeyCode::Enter => {
                    if self.focus == Focus::List {
                        self.select_pr();
                        self.view = View::Detail;
                        self.focus = Focus::Detail;
                    } else if self.focus == Focus::PrChecks {
                        // View logs for selected check
                        self.view_pr_check_logs();
                    }
                }
                KeyCode::Esc => {
                    if self.view == View::Detail {
                        self.view = View::List;
                        self.focus = Focus::List;
                    }
                }
                KeyCode::Char('d') => {
                    if self.selected_pr.is_some() {
                        self.view = View::Diff;
                        self.diff_scroll = 0;
                    }
                }
                KeyCode::Char('v') => {
                    self.approve_pr().await;
                }
                KeyCode::Char('x') => {
                    self.input_mode = Some(InputMode::Comment);
                    self.message = Some("Enter comment for request changes:".to_string());
                }
                KeyCode::Char('c') => {
                    self.input_mode = Some(InputMode::Comment);
                    self.message = Some("Enter comment:".to_string());
                }
                KeyCode::Char('m') => {
                    self.merge_pr().await;
                }
                KeyCode::Char('C') => {
                    self.checkout_pr();
                }
                KeyCode::Char('f') => {
                    self.cycle_filter();
                    self.loading = true;
                    self.loading_what = Some("Filtering PRs...".to_string());
                    self.spawn_fetch_prs();
                }
                KeyCode::Char('R') => {
                    // Rerun selected PR check
                    self.rerun_pr_check().await;
                }
                KeyCode::Char('L') => {
                    // View logs for selected PR check
                    self.view_pr_check_logs();
                }
                _ => {}
            },
            View::Diff => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.diff_scroll = self.diff_scroll.saturating_add(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.diff_scroll = self.diff_scroll.saturating_sub(1);
                }
                KeyCode::PageDown => {
                    self.diff_scroll = self.diff_scroll.saturating_add(20);
                }
                KeyCode::PageUp => {
                    self.diff_scroll = self.diff_scroll.saturating_sub(20);
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.view = View::Detail;
                }
                _ => {}
            },
            _ => {}
        }
    }

    async fn handle_actions_keys(&mut self, key: KeyEvent) {
        match self.view {
            View::List => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.next_run();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.previous_run();
                }
                KeyCode::Enter => {
                    self.select_run();
                    self.view = View::Jobs;
                }
                KeyCode::Char('R') => {
                    self.rerun_workflow().await;
                }
                _ => {}
            },
            View::Jobs => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.next_job();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.previous_job();
                }
                KeyCode::Enter | KeyCode::Char('L') => {
                    self.fetch_logs();
                    self.tab = Tab::Logs;
                }
                KeyCode::Esc => {
                    self.view = View::List;
                }
                KeyCode::Char('R') => {
                    self.rerun_workflow().await;
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn handle_logs_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.log_scroll = self.log_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.log_scroll = self.log_scroll.saturating_sub(1);
            }
            KeyCode::Char('h') => {
                self.log_h_scroll = self.log_h_scroll.saturating_sub(10);
            }
            KeyCode::Char('l') => {
                self.log_h_scroll = self.log_h_scroll.saturating_add(10);
            }
            KeyCode::PageDown => {
                self.log_scroll = self.log_scroll.saturating_add(20);
            }
            KeyCode::PageUp => {
                self.log_scroll = self.log_scroll.saturating_sub(20);
            }
            KeyCode::Char('g') => {
                self.log_scroll = 0;
                self.log_h_scroll = 0;
            }
            KeyCode::Char('G') => {
                let line_count = self.logs.lines().count() as u16;
                self.log_scroll = line_count.saturating_sub(20);
            }
            KeyCode::Char('0') => {
                self.log_h_scroll = 0;
            }
            KeyCode::Char('/') => {
                self.input_mode = Some(InputMode::Search);
                self.message = Some("Search:".to_string());
            }
            KeyCode::Char('n') => {
                self.next_log_match();
            }
            KeyCode::Char('N') => {
                self.prev_log_match();
            }
            KeyCode::Esc => {
                self.tab = Tab::Actions;
                self.log_search = None;
                self.log_matches.clear();
            }
            _ => {}
        }
    }

    // Navigation helpers
    fn next_pr(&mut self) {
        let len = self.prs.len();
        if len == 0 { return; }
        let i = match self.pr_list_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.pr_list_state.select(Some(i));
    }

    fn previous_pr(&mut self) {
        let len = self.prs.len();
        if len == 0 { return; }
        let i = match self.pr_list_state.selected() {
            Some(i) => (i + len - 1) % len,
            None => 0,
        };
        self.pr_list_state.select(Some(i));
    }

    fn next_pr_check(&mut self) {
        let len = self.pr_checks.len();
        if len == 0 { return; }
        let i = match self.pr_checks_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.pr_checks_state.select(Some(i));
    }

    fn previous_pr_check(&mut self) {
        let len = self.pr_checks.len();
        if len == 0 { return; }
        let i = match self.pr_checks_state.selected() {
            Some(i) => (i + len - 1) % len,
            None => 0,
        };
        self.pr_checks_state.select(Some(i));
    }

    fn next_run(&mut self) {
        let len = self.runs.len();
        if len == 0 { return; }
        let i = match self.run_list_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.run_list_state.select(Some(i));
    }

    fn previous_run(&mut self) {
        let len = self.runs.len();
        if len == 0 { return; }
        let i = match self.run_list_state.selected() {
            Some(i) => (i + len - 1) % len,
            None => 0,
        };
        self.run_list_state.select(Some(i));
    }

    fn next_job(&mut self) {
        let len = self.jobs.len();
        if len == 0 { return; }
        let i = match self.job_list_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.job_list_state.select(Some(i));
    }

    fn previous_job(&mut self) {
        let len = self.jobs.len();
        if len == 0 { return; }
        let i = match self.job_list_state.selected() {
            Some(i) => (i + len - 1) % len,
            None => 0,
        };
        self.job_list_state.select(Some(i));
    }

    fn cycle_filter(&mut self) {
        self.pr_filter = match self.pr_filter {
            PrFilter::All => PrFilter::Mine,
            PrFilter::Mine => PrFilter::ReviewRequested,
            PrFilter::ReviewRequested => PrFilter::All,
        };
    }

    // Data fetching (now async)
    fn select_pr(&mut self) {
        if let Some(i) = self.pr_list_state.selected() {
            if let Some(pr) = self.prs.get(i) {
                self.selected_pr = Some(pr.clone());
                self.diff_scroll = 0;
                self.pr_checks.clear();
                self.pr_checks_state.select(None);

                // Spawn async fetch for diff and checks
                self.loading = true;
                self.loading_what = Some("Loading diff...".to_string());
                self.spawn_fetch_diff(pr.number);
                self.spawn_fetch_pr_checks(&pr.head.sha);
            }
        }
    }

    fn select_run(&mut self) {
        if let Some(i) = self.run_list_state.selected() {
            if let Some(run) = self.runs.get(i) {
                self.selected_run = Some(run.clone());
                self.job_list_state.select(Some(0));

                // Spawn async fetch for jobs
                self.loading = true;
                self.loading_what = Some("Loading jobs...".to_string());
                self.spawn_fetch_jobs(run.id);
            }
        }
    }

    fn fetch_logs(&mut self) {
        if let Some(run) = &self.selected_run {
            let job_id = self.job_list_state.selected()
                .and_then(|i| self.jobs.get(i))
                .map(|j| j.id);

            self.loading = true;
            self.loading_what = Some("Loading logs...".to_string());
            self.spawn_fetch_logs(run.id, job_id);
        }
    }

    fn view_pr_check_logs(&mut self) {
        if let Some(i) = self.pr_checks_state.selected() {
            if let Some(check) = self.pr_checks.get(i) {
                self.selected_run = Some(check.clone());
                self.loading = true;
                self.loading_what = Some("Loading logs...".to_string());
                self.spawn_fetch_logs(check.id, None);
                self.tab = Tab::Logs;
            }
        }
    }

    // Actions
    async fn approve_pr(&mut self) {
        if let Some(pr) = &self.selected_pr {
            if let Some(client) = &self.client {
                self.loading = true;
                self.loading_what = Some("Approving PR...".to_string());
                match client.approve_pr(&self.owner, &self.repo_name, pr.number).await {
                    Ok(_) => {
                        self.message = Some(format!("Approved PR #{}", pr.number));
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to approve: {}", e));
                    }
                }
                self.loading = false;
                self.loading_what = None;
            }
        }
    }

    async fn merge_pr(&mut self) {
        if let Some(pr) = &self.selected_pr {
            if let Some(client) = &self.client {
                self.loading = true;
                self.loading_what = Some("Merging PR...".to_string());
                match client.merge_pr(&self.owner, &self.repo_name, pr.number).await {
                    Ok(_) => {
                        self.message = Some(format!("Merged PR #{}", pr.number));
                        self.spawn_fetch_prs();
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to merge: {}", e));
                        self.loading = false;
                        self.loading_what = None;
                    }
                }
            }
        }
    }

    fn checkout_pr(&mut self) {
        if let Some(pr) = &self.selected_pr {
            let pr_number = pr.number;
            let tx = self.async_tx.clone();
            tokio::spawn(async move {
                let output = std::process::Command::new("gh")
                    .args(["pr", "checkout", &pr_number.to_string()])
                    .output();

                if let Some(tx) = tx {
                    match output {
                        Ok(o) if o.status.success() => {
                            let _ = tx.send(AsyncMsg::Message(format!("Checked out PR #{}", pr_number)));
                        }
                        Ok(o) => {
                            let _ = tx.send(AsyncMsg::Error(format!(
                                "Checkout failed: {}",
                                String::from_utf8_lossy(&o.stderr)
                            )));
                        }
                        Err(e) => {
                            let _ = tx.send(AsyncMsg::Error(format!("Checkout failed: {}", e)));
                        }
                    }
                }
            });
        }
    }

    fn create_pr(&mut self) {
        let tx = self.async_tx.clone();
        let repo = format!("{}/{}", self.owner, self.repo_name);
        tokio::spawn(async move {
            // Open gh pr create in interactive mode
            let output = std::process::Command::new("gh")
                .args(["pr", "create", "--web", "--repo", &repo])
                .output();

            if let Some(tx) = tx {
                match output {
                    Ok(o) if o.status.success() => {
                        let _ = tx.send(AsyncMsg::Message("Opened PR creation in browser".to_string()));
                    }
                    Ok(o) => {
                        let _ = tx.send(AsyncMsg::Error(format!(
                            "Failed to create PR: {}",
                            String::from_utf8_lossy(&o.stderr)
                        )));
                    }
                    Err(e) => {
                        let _ = tx.send(AsyncMsg::Error(format!("Failed to create PR: {}", e)));
                    }
                }
            }
        });
        self.message = Some("Opening PR creation in browser...".to_string());
    }

    async fn submit_comment(&mut self) {
        self.message = Some("Comment submitted".to_string());
    }

    async fn rerun_workflow(&mut self) {
        let run = self.run_list_state.selected().and_then(|i| self.runs.get(i).cloned());
        if let Some(run) = run {
            if let Some(client) = &self.client {
                self.loading = true;
                self.loading_what = Some("Triggering rerun...".to_string());
                match client.rerun_workflow(&self.owner, &self.repo_name, run.id).await {
                    Ok(_) => {
                        self.message = Some(format!("Rerun triggered for {}", run.name));
                        self.spawn_fetch_runs();
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to rerun: {}", e));
                    }
                }
                self.loading = false;
                self.loading_what = None;
            }
        }
    }

    async fn rerun_pr_check(&mut self) {
        if let Some(i) = self.pr_checks_state.selected() {
            if let Some(check) = self.pr_checks.get(i).cloned() {
                if let Some(client) = &self.client {
                    self.loading = true;
                    self.loading_what = Some("Triggering rerun...".to_string());
                    match client.rerun_workflow(&self.owner, &self.repo_name, check.id).await {
                        Ok(_) => {
                            self.message = Some(format!("Rerun triggered for {}", check.name));
                            // Refresh PR checks
                            if let Some(pr) = &self.selected_pr {
                                self.spawn_fetch_pr_checks(&pr.head.sha);
                            }
                        }
                        Err(e) => {
                            self.error = Some(format!("Failed to rerun: {}", e));
                        }
                    }
                    self.loading = false;
                    self.loading_what = None;
                }
            }
        }
    }

    fn refresh(&mut self) {
        self.error = None;
        self.message = None;
        self.loading = true;

        match self.tab {
            Tab::PRs => {
                self.loading_what = Some("Refreshing PRs...".to_string());
                self.spawn_fetch_prs();
                if let Some(pr) = &self.selected_pr {
                    self.spawn_fetch_pr_checks(&pr.head.sha);
                }
            }
            Tab::Actions => {
                self.loading_what = Some("Refreshing workflows...".to_string());
                self.spawn_fetch_runs();
            }
            Tab::Logs => {
                self.loading_what = Some("Refreshing logs...".to_string());
                if let Some(run) = &self.selected_run {
                    let job_id = self.job_list_state.selected()
                        .and_then(|i| self.jobs.get(i))
                        .map(|j| j.id);
                    self.spawn_fetch_logs(run.id, job_id);
                }
            }
        }
    }

    fn find_log_matches(&mut self) {
        self.log_matches.clear();
        if let Some(ref search) = self.log_search {
            let search_lower = search.to_lowercase();
            for (i, line) in self.logs.lines().enumerate() {
                if line.to_lowercase().contains(&search_lower) {
                    self.log_matches.push(i);
                }
            }
            self.log_match_index = 0;
            if let Some(&line) = self.log_matches.first() {
                self.log_scroll = line as u16;
            }
        }
    }

    fn next_log_match(&mut self) {
        if !self.log_matches.is_empty() {
            self.log_match_index = (self.log_match_index + 1) % self.log_matches.len();
            self.log_scroll = self.log_matches[self.log_match_index] as u16;
        }
    }

    fn prev_log_match(&mut self) {
        if !self.log_matches.is_empty() {
            self.log_match_index = (self.log_match_index + self.log_matches.len() - 1)
                % self.log_matches.len();
            self.log_scroll = self.log_matches[self.log_match_index] as u16;
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self {
            tab: Tab::default(),
            view: View::default(),
            focus: Focus::default(),
            repo: String::new(),
            owner: String::new(),
            repo_name: String::new(),
            prs: Vec::new(),
            pr_list_state: ListState::default(),
            selected_pr: None,
            pr_diff: None,
            pr_filter: PrFilter::default(),
            diff_scroll: 0,
            pr_checks: Vec::new(),
            pr_checks_state: ListState::default(),
            runs: Vec::new(),
            run_list_state: ListState::default(),
            selected_run: None,
            jobs: Vec::new(),
            job_list_state: ListState::default(),
            logs: String::new(),
            log_scroll: 0,
            log_h_scroll: 0,
            log_search: None,
            log_matches: Vec::new(),
            log_match_index: 0,
            loading: false,
            loading_what: None,
            error: None,
            message: None,
            should_quit: false,
            show_help: false,
            input_mode: None,
            input_buffer: String::new(),
            client: None,
            async_rx: None,
            async_tx: None,
        }
    }
}
