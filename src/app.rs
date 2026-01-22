use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::ListState;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::event::{Event, EventHandler};
use crate::github::types::{Commit, Job, PullRequest, Review, WorkflowRun};
use crate::github::Client;
use crate::ui;
use crate::ui::MatrixRain;

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

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum DiffMode {
    #[default]
    Full,
    ByCommit,
}

// Messages for async operations
pub enum AsyncMsg {
    UserLoaded(String),
    PrsLoaded(Vec<PullRequest>),
    RunsLoaded(Vec<WorkflowRun>),
    DiffLoaded(String),
    PrChecksLoaded(Vec<WorkflowRun>),
    ReviewsLoaded(Vec<Review>),
    JobsLoaded(Vec<Job>),
    LogsLoaded(String),
    CommitsLoaded(Vec<Commit>),
    CommitDiffLoaded(String),
    Error(String),
    Message(String),
}

#[derive(Default)]
pub struct App {
    pub tab: Tab,
    pub view: View,
    pub focus: Focus,
    pub repo: String,
    pub owner: String,
    pub repo_name: String,
    pub current_user: Option<String>,

    // PR state
    pub all_prs: Vec<PullRequest>,  // All PRs from API
    pub prs: Vec<PullRequest>,       // Filtered PRs for display
    pub pr_list_state: ListState,
    pub selected_pr: Option<PullRequest>,
    pub pr_diff: Option<String>,
    pub pr_filter: PrFilter,
    pub diff_scroll: u16,

    // PR checks (workflow runs for selected PR)
    pub pr_checks: Vec<WorkflowRun>,
    pub pr_checks_state: ListState,

    // PR reviews (approval status)
    pub pr_reviews: Vec<Review>,

    // Commit review mode
    pub diff_mode: DiffMode,
    pub pr_commits: Vec<Commit>,
    pub pr_commits_state: ListState,
    pub commit_diff: Option<String>,

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
    pub status_message: Option<StatusMessage>,
    pub should_quit: bool,
    pub show_help: bool,
    pub input_mode: Option<InputMode>,
    pub input_buffer: String,

    // Matrix rain animation
    pub matrix_rain: MatrixRain,

    // Initial PR to select (from CLI argument)
    pub initial_pr: Option<u64>,

    // GitHub client
    pub client: Option<Client>,

    // Async message channel
    async_rx: Option<mpsc::UnboundedReceiver<AsyncMsg>>,
    async_tx: Option<mpsc::UnboundedSender<AsyncMsg>>,
}

/// Status bar message with explicit lifetime semantics
#[derive(Clone)]
pub enum StatusMessage {
    /// Auto-dismissing notification - expires after the specified instant
    Notification { text: String, expires_at: Instant },
    /// Persistent prompt - stays until manually cleared (e.g., input mode prompts)
    Prompt(String),
}

impl StatusMessage {
    /// Create a notification that auto-dismisses after the given duration
    pub fn notification(text: impl Into<String>, duration: Duration) -> Self {
        StatusMessage::Notification {
            text: text.into(),
            expires_at: Instant::now() + duration,
        }
    }

    /// Create a persistent prompt
    pub fn prompt(text: impl Into<String>) -> Self {
        StatusMessage::Prompt(text.into())
    }

    /// Get the message text
    pub fn text(&self) -> &str {
        match self {
            StatusMessage::Notification { text, .. } => text,
            StatusMessage::Prompt(text) => text,
        }
    }

    /// Check if this message has expired
    pub fn is_expired(&self) -> bool {
        match self {
            StatusMessage::Notification { expires_at, .. } => Instant::now() > *expires_at,
            StatusMessage::Prompt(_) => false, // Prompts never expire
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Search,
    Comment,
    EditTitle,
    AddLabel,
    AddReviewer,
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
            pr_commits_state: ListState::default(),
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

        self.spawn_fetch_current_user();
        self.spawn_fetch_prs();
        self.spawn_fetch_runs();

        // Event loop
        let mut events = EventHandler::new(Duration::from_millis(100));

        while !self.should_quit {
            // Process async messages
            self.process_async_messages();

            // Auto-dismiss expired status messages BEFORE drawing
            if let Some(ref msg) = self.status_message {
                if msg.is_expired() {
                    self.status_message = None;
                }
            }

            terminal.draw(|f| ui::render(f, self))?;

            if let Some(event) = events.next().await {
                match event {
                    Event::Tick => {
                        // Process any pending async messages
                        self.process_async_messages();
                        // Advance matrix rain animation when loading
                        if self.loading {
                            self.matrix_rain.tick();
                        }
                    }
                    Event::Key(key) => self.handle_key(key).await,
                    Event::Resize(w, h) => {
                        self.matrix_rain.resize(w, h);
                    }
                }
            }
        }

        Ok(())
    }

    fn process_async_messages(&mut self) {
        // Collect messages first to avoid borrow issues
        let messages: Vec<AsyncMsg> = if let Some(ref mut rx) = self.async_rx {
            let mut msgs = Vec::new();
            while let Ok(msg) = rx.try_recv() {
                msgs.push(msg);
            }
            msgs
        } else {
            Vec::new()
        };

        let mut needs_filter = false;
        let mut needs_select_pr: Option<u64> = None;

        for msg in messages {
            match msg {
                AsyncMsg::UserLoaded(user) => {
                    self.current_user = Some(user);
                    needs_filter = true;
                }
                AsyncMsg::PrsLoaded(prs) => {
                    self.all_prs = prs;
                    needs_filter = true;
                    self.loading = false;
                    self.loading_what = None;

                    // Handle initial PR selection from CLI argument
                    if let Some(pr_number) = self.initial_pr.take() {
                        needs_select_pr = Some(pr_number);
                    }
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
                AsyncMsg::ReviewsLoaded(reviews) => {
                    self.pr_reviews = reviews;
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
                AsyncMsg::CommitsLoaded(commits) => {
                    self.pr_commits = commits;
                    if !self.pr_commits.is_empty() && self.pr_commits_state.selected().is_none() {
                        self.pr_commits_state.select(Some(0));
                    }
                }
                AsyncMsg::CommitDiffLoaded(diff) => {
                    self.commit_diff = Some(diff);
                    self.diff_scroll = 0;
                    self.loading = false;
                    self.loading_what = None;
                }
                AsyncMsg::Error(e) => {
                    self.error = Some(e);
                    self.loading = false;
                    self.loading_what = None;
                }
                AsyncMsg::Message(m) => {
                    self.set_message(m);
                }
            }
        }

        if needs_filter {
            self.apply_pr_filter();
        }

        // Select initial PR if specified
        if let Some(pr_number) = needs_select_pr {
            self.select_pr_by_number(pr_number);
        }
    }

    // Spawn async tasks for fetching data
    fn spawn_fetch_current_user(&self) {
        if let (Some(client), Some(tx)) = (self.client.clone(), self.async_tx.clone()) {
            tokio::spawn(async move {
                match client.get_current_user().await {
                    Ok(user) => {
                        let _ = tx.send(AsyncMsg::UserLoaded(user));
                    }
                    Err(_) => {
                        // Silently ignore - filter will just show all PRs
                    }
                }
            });
        }
    }

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

    fn spawn_fetch_reviews(&self, pr_number: u64) {
        if let (Some(client), Some(tx)) = (self.client.clone(), self.async_tx.clone()) {
            let owner = self.owner.clone();
            let repo = self.repo_name.clone();
            tokio::spawn(async move {
                match client.list_pr_reviews(&owner, &repo, pr_number).await {
                    Ok(reviews) => { let _ = tx.send(AsyncMsg::ReviewsLoaded(reviews)); }
                    Err(e) => { let _ = tx.send(AsyncMsg::Error(format!("Failed to fetch reviews: {}", e))); }
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

    fn spawn_fetch_commits(&self, pr_number: u64) {
        if let (Some(client), Some(tx)) = (self.client.clone(), self.async_tx.clone()) {
            let owner = self.owner.clone();
            let repo = self.repo_name.clone();
            tokio::spawn(async move {
                match client.list_pr_commits(&owner, &repo, pr_number).await {
                    Ok(commits) => { let _ = tx.send(AsyncMsg::CommitsLoaded(commits)); }
                    Err(e) => { let _ = tx.send(AsyncMsg::Error(format!("Failed to fetch commits: {}", e))); }
                }
            });
        }
    }

    fn spawn_fetch_commit_diff(&self, sha: &str) {
        if let (Some(client), Some(tx)) = (self.client.clone(), self.async_tx.clone()) {
            let owner = self.owner.clone();
            let repo = self.repo_name.clone();
            let sha = sha.to_string();
            tokio::spawn(async move {
                match client.get_commit_diff(&owner, &repo, &sha).await {
                    Ok(diff) => { let _ = tx.send(AsyncMsg::CommitDiffLoaded(diff)); }
                    Err(e) => { let _ = tx.send(AsyncMsg::Error(format!("Failed to fetch commit diff: {}", e))); }
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
                    self.status_message = None;
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
                        InputMode::EditTitle => {
                            self.submit_edit_title().await;
                        }
                        InputMode::AddLabel => {
                            self.submit_add_label().await;
                        }
                        InputMode::AddReviewer => {
                            self.submit_add_reviewer().await;
                        }
                    }
                    self.input_mode = None;
                    self.input_buffer.clear();
                    self.status_message = None;
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    // Limit input buffer to prevent unbounded memory usage
                    if self.input_buffer.len() < 1024 {
                        self.input_buffer.push(c);
                    }
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
            KeyCode::Tab => {
                // Cycle through tabs: PRs -> Actions -> Logs -> PRs
                self.tab = match self.tab {
                    Tab::PRs => Tab::Actions,
                    Tab::Actions => Tab::Logs,
                    Tab::Logs => Tab::PRs,
                };
                self.view = View::List;
                return;
            }
            KeyCode::BackTab => {
                // Reverse cycle: PRs -> Logs -> Actions -> PRs
                self.tab = match self.tab {
                    Tab::PRs => Tab::Logs,
                    Tab::Actions => Tab::PRs,
                    Tab::Logs => Tab::Actions,
                };
                self.view = View::List;
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
                KeyCode::Char('o') => {
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
                        self.view_pr_check_jobs();
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
                    self.status_message = Some(StatusMessage::prompt("Enter comment for request changes:"));
                }
                KeyCode::Char('c') => {
                    self.input_mode = Some(InputMode::Comment);
                    self.status_message = Some(StatusMessage::prompt("Enter comment:"));
                }
                KeyCode::Char('m') => {
                    self.merge_pr().await;
                }
                KeyCode::Char('C') => {
                    self.checkout_pr();
                }
                KeyCode::Char('f') => {
                    self.cycle_filter();
                }
                KeyCode::Char('R') => {
                    // Rerun selected PR check
                    self.rerun_pr_check().await;
                }
                KeyCode::Char('L') => {
                    // View logs for selected PR check
                    self.view_pr_check_jobs();
                }
                KeyCode::Char('e') => {
                    // Edit PR title
                    if self.selected_pr.is_some() {
                        self.input_mode = Some(InputMode::EditTitle);
                        self.input_buffer = self.selected_pr.as_ref().map(|p| p.title.clone()).unwrap_or_default();
                        self.status_message = Some(StatusMessage::prompt("Edit PR title:"));
                    }
                }
                KeyCode::Char('a') => {
                    // Add reviewer
                    if self.selected_pr.is_some() {
                        self.input_mode = Some(InputMode::AddReviewer);
                        self.status_message = Some(StatusMessage::prompt("Add reviewer (username):"));
                    }
                }
                KeyCode::Char('b') => {
                    // Add label
                    if self.selected_pr.is_some() {
                        self.input_mode = Some(InputMode::AddLabel);
                        self.status_message = Some(StatusMessage::prompt("Add label:"));
                    }
                }
                KeyCode::Char('w') => {
                    // Open PR in browser
                    self.open_pr_in_browser();
                }
                KeyCode::Char('y') => {
                    // Copy branch name to clipboard
                    self.copy_branch_to_clipboard();
                }
                KeyCode::Char('Y') => {
                    // Copy checkout command to clipboard
                    self.copy_checkout_command_to_clipboard();
                }
                KeyCode::Char('u') => {
                    // Copy PR URL to clipboard
                    self.copy_pr_url_to_clipboard();
                }
                KeyCode::Char('p') => {
                    // Toggle diff mode (Full <-> ByCommit)
                    self.toggle_diff_mode();
                }
                KeyCode::Char('[') => {
                    // Previous commit (in commit mode)
                    if self.diff_mode == DiffMode::ByCommit {
                        self.previous_commit();
                        self.load_selected_commit_diff();
                    }
                }
                KeyCode::Char(']') => {
                    // Next commit (in commit mode)
                    if self.diff_mode == DiffMode::ByCommit {
                        self.next_commit();
                        self.load_selected_commit_diff();
                    }
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
                self.status_message = Some(StatusMessage::prompt("Search:"));
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

    fn next_commit(&mut self) {
        let len = self.pr_commits.len();
        if len == 0 { return; }
        let i = match self.pr_commits_state.selected() {
            Some(i) => (i + 1).min(len - 1),
            None => 0,
        };
        self.pr_commits_state.select(Some(i));
    }

    fn previous_commit(&mut self) {
        let len = self.pr_commits.len();
        if len == 0 { return; }
        let i = match self.pr_commits_state.selected() {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.pr_commits_state.select(Some(i));
    }

    fn toggle_diff_mode(&mut self) {
        self.diff_mode = match self.diff_mode {
            DiffMode::Full => {
                // Switch to commit mode
                if !self.pr_commits.is_empty() {
                    if self.pr_commits_state.selected().is_none() {
                        self.pr_commits_state.select(Some(0));
                    }
                    self.load_selected_commit_diff();
                    DiffMode::ByCommit
                } else {
                    self.set_message("No commits found for this PR");
                    DiffMode::Full
                }
            }
            DiffMode::ByCommit => {
                self.diff_scroll = 0;
                DiffMode::Full
            }
        };
    }

    fn load_selected_commit_diff(&mut self) {
        if let Some(i) = self.pr_commits_state.selected() {
            if let Some(commit) = self.pr_commits.get(i) {
                self.loading = true;
                self.loading_what = Some(format!("Loading commit {}...", commit.short_sha()));
                self.spawn_fetch_commit_diff(&commit.sha);
            }
        }
    }

    fn cycle_filter(&mut self) {
        self.pr_filter = match self.pr_filter {
            PrFilter::All => PrFilter::Mine,
            PrFilter::Mine => PrFilter::ReviewRequested,
            PrFilter::ReviewRequested => PrFilter::All,
        };
        self.apply_pr_filter();
    }

    fn apply_pr_filter(&mut self) {
        let current_user = self.current_user.as_deref();

        self.prs = match self.pr_filter {
            PrFilter::All => self.all_prs.clone(),
            PrFilter::Mine => {
                if let Some(user) = current_user {
                    self.all_prs
                        .iter()
                        .filter(|pr| pr.user.login == user)
                        .cloned()
                        .collect()
                } else {
                    self.all_prs.clone()
                }
            }
            PrFilter::ReviewRequested => {
                if let Some(user) = current_user {
                    self.all_prs
                        .iter()
                        .filter(|pr| pr.requested_reviewers.iter().any(|r| r.login == user))
                        .cloned()
                        .collect()
                } else {
                    self.all_prs.clone()
                }
            }
        };

        // Reset selection if needed
        if self.prs.is_empty() {
            self.pr_list_state.select(None);
        } else if self
            .pr_list_state
            .selected()
            .is_none_or(|idx| idx >= self.prs.len())
        {
            self.pr_list_state.select(Some(0));
        }
    }

    fn select_pr_by_number(&mut self, pr_number: u64) {
        // Find the PR in the filtered list
        if let Some(idx) = self.prs.iter().position(|pr| pr.number == pr_number) {
            self.pr_list_state.select(Some(idx));
            self.select_pr();
            self.view = View::Detail;
            self.focus = Focus::Detail;
        } else {
            self.error = Some(format!("PR #{} not found in current filter", pr_number));
        }
    }

    // Data fetching (now async)
    fn select_pr(&mut self) {
        if let Some(i) = self.pr_list_state.selected() {
            if let Some(pr) = self.prs.get(i) {
                self.selected_pr = Some(pr.clone());
                self.diff_scroll = 0;
                self.pr_checks.clear();
                self.pr_checks_state.select(None);
                self.pr_reviews.clear();
                self.pr_commits.clear();
                self.pr_commits_state.select(None);
                self.commit_diff = None;
                self.diff_mode = DiffMode::Full;

                // Spawn async fetch for diff, checks, reviews, and commits
                self.loading = true;
                self.loading_what = Some("Loading diff...".to_string());
                self.spawn_fetch_diff(pr.number);
                self.spawn_fetch_pr_checks(&pr.head.sha);
                self.spawn_fetch_reviews(pr.number);
                self.spawn_fetch_commits(pr.number);
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

    fn view_pr_check_jobs(&mut self) {
        if let Some(i) = self.pr_checks_state.selected() {
            if let Some(check) = self.pr_checks.get(i) {
                self.selected_run = Some(check.clone());
                self.job_list_state.select(Some(0));

                // Find and select this run in the runs list
                if let Some(run_idx) = self.runs.iter().position(|r| r.id == check.id) {
                    self.run_list_state.select(Some(run_idx));
                } else {
                    // Run not in list - add it at the top and select it
                    self.runs.insert(0, check.clone());
                    self.run_list_state.select(Some(0));
                }

                self.loading = true;
                self.loading_what = Some("Loading jobs...".to_string());
                self.spawn_fetch_jobs(check.id);
                self.tab = Tab::Actions;
                self.view = View::Jobs;
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
                        self.set_message(format!("Approved PR #{}", pr.number));
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
                        self.set_message(format!("Merged PR #{}", pr.number));
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
        self.set_message("Opening PR creation in browser...");
    }

    async fn submit_comment(&mut self) {
        self.set_message("Comment submitted");
    }

    async fn submit_edit_title(&mut self) {
        let pr_number = match &self.selected_pr {
            Some(pr) => pr.number,
            None => return,
        };

        let new_title = self.input_buffer.clone();
        if new_title.is_empty() {
            self.error = Some("Title cannot be empty".to_string());
            return;
        }

        if let Some(client) = &self.client {
            self.loading = true;
            self.loading_what = Some("Updating title...".to_string());
            match client.edit_pr_title(&self.owner, &self.repo_name, pr_number, &new_title).await {
                Ok(_) => {
                    self.set_message(format!("Updated PR #{} title", pr_number));
                    // Update local state
                    if let Some(ref mut pr) = self.selected_pr {
                        pr.title = new_title.clone();
                    }
                    // Also update in lists
                    for p in &mut self.all_prs {
                        if p.number == pr_number {
                            p.title = new_title.clone();
                        }
                    }
                    self.apply_pr_filter();
                }
                Err(e) => {
                    self.error = Some(format!("Failed to update title: {}", e));
                }
            }
            self.loading = false;
            self.loading_what = None;
        }
    }

    async fn submit_add_label(&mut self) {
        let pr_number = match &self.selected_pr {
            Some(pr) => pr.number,
            None => return,
        };

        let label = self.input_buffer.trim().to_string();
        if label.is_empty() {
            self.error = Some("Label cannot be empty".to_string());
            return;
        }

        if let Some(client) = &self.client {
            self.loading = true;
            self.loading_what = Some("Adding label...".to_string());
            match client.add_pr_labels(&self.owner, &self.repo_name, pr_number, &[label.as_str()]).await {
                Ok(_) => {
                    self.set_message(format!("Added label '{}' to PR #{}", label, pr_number));
                    // Refresh PRs to get updated labels
                    self.spawn_fetch_prs();
                }
                Err(e) => {
                    self.error = Some(format!("Failed to add label: {}", e));
                }
            }
            self.loading = false;
            self.loading_what = None;
        }
    }

    async fn submit_add_reviewer(&mut self) {
        let pr_number = match &self.selected_pr {
            Some(pr) => pr.number,
            None => return,
        };

        let reviewer = self.input_buffer.trim().to_string();
        if reviewer.is_empty() {
            self.error = Some("Reviewer username cannot be empty".to_string());
            return;
        }

        if let Some(client) = &self.client {
            self.loading = true;
            self.loading_what = Some("Adding reviewer...".to_string());
            match client.add_pr_reviewers(&self.owner, &self.repo_name, pr_number, &[reviewer.as_str()]).await {
                Ok(_) => {
                    self.set_message(format!("Added '{}' as reviewer to PR #{}", reviewer, pr_number));
                    // Refresh PRs to get updated reviewers
                    self.spawn_fetch_prs();
                }
                Err(e) => {
                    self.error = Some(format!("Failed to add reviewer: {}", e));
                }
            }
            self.loading = false;
            self.loading_what = None;
        }
    }

    fn copy_branch_to_clipboard(&mut self) {
        if let Some(pr) = &self.selected_pr {
            let branch = &pr.head.ref_name;
            if Self::copy_to_clipboard(branch) {
                self.set_message(format!("Copied branch: {}", branch));
            } else {
                self.error = Some("Failed to copy to clipboard".to_string());
            }
        }
    }

    fn copy_checkout_command_to_clipboard(&mut self) {
        if let Some(pr) = &self.selected_pr {
            let cmd = format!("git checkout {}", pr.head.ref_name);
            if Self::copy_to_clipboard(&cmd) {
                self.set_message(format!("Copied: {}", cmd));
            } else {
                self.error = Some("Failed to copy to clipboard".to_string());
            }
        }
    }

    fn copy_pr_url_to_clipboard(&mut self) {
        if let Some(pr) = &self.selected_pr {
            let url = format!("https://github.com/{}/pull/{}", self.repo, pr.number);
            if Self::copy_to_clipboard(&url) {
                self.set_message(format!("Copied: {}", url));
            } else {
                self.error = Some("Failed to copy to clipboard".to_string());
            }
        }
    }

    fn copy_to_clipboard(text: &str) -> bool {
        // Try different clipboard commands based on platform
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(stdin) = child.stdin.as_mut() {
                        stdin.write_all(text.as_bytes())?;
                    }
                    child.wait()
                })
                .map(|s| s.success())
                .unwrap_or(false)
        }

        #[cfg(target_os = "linux")]
        {
            // Try xclip first, then xsel
            std::process::Command::new("xclip")
                .args(["-selection", "clipboard"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(stdin) = child.stdin.as_mut() {
                        stdin.write_all(text.as_bytes())?;
                    }
                    child.wait()
                })
                .map(|s| s.success())
                .unwrap_or(false)
        }

        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("clip")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(stdin) = child.stdin.as_mut() {
                        stdin.write_all(text.as_bytes())?;
                    }
                    child.wait()
                })
                .map(|s| s.success())
                .unwrap_or(false)
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            false
        }
    }

    fn open_pr_in_browser(&mut self) {
        if let Some(pr) = &self.selected_pr {
            let pr_number = pr.number;
            let owner = self.owner.clone();
            let repo = self.repo_name.clone();
            let tx = self.async_tx.clone();
            tokio::spawn(async move {
                let output = tokio::process::Command::new("gh")
                    .args([
                        "pr", "view",
                        &pr_number.to_string(),
                        "--repo", &format!("{}/{}", owner, repo),
                        "--web",
                    ])
                    .output()
                    .await;

                if let Some(tx) = tx {
                    match output {
                        Ok(o) if o.status.success() => {
                            let _ = tx.send(AsyncMsg::Message(format!("Opened PR #{} in browser", pr_number)));
                        }
                        Ok(o) => {
                            let _ = tx.send(AsyncMsg::Error(format!(
                                "Failed to open PR: {}",
                                String::from_utf8_lossy(&o.stderr)
                            )));
                        }
                        Err(e) => {
                            let _ = tx.send(AsyncMsg::Error(format!("Failed to open PR: {}", e)));
                        }
                    }
                }
            });
            self.set_message("Opening PR in browser...");
        }
    }

    async fn rerun_workflow(&mut self) {
        let run = self.run_list_state.selected().and_then(|i| self.runs.get(i).cloned());
        if let Some(run) = run {
            if let Some(client) = &self.client {
                self.loading = true;
                self.loading_what = Some("Triggering rerun...".to_string());
                match client.rerun_workflow(&self.owner, &self.repo_name, run.id).await {
                    Ok(_) => {
                        self.set_message(format!("Rerun triggered for {}", run.name));
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
                            self.set_message(format!("Rerun triggered for {}", check.name));
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
        self.status_message = None;
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

    /// Set a notification message that auto-dismisses after 3 seconds
    fn set_message(&mut self, msg: impl Into<String>) {
        self.status_message = Some(StatusMessage::notification(msg, Duration::from_secs(3)));
    }
}

