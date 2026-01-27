use anyhow::{Context, Result};
use octocrab::Octocrab;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::types::{Commit, Job, PullRequest, Review, WorkflowRun};

const API_BASE: &str = "https://api.github.com";

/// In-memory cache for immutable data
#[derive(Default)]
struct Cache {
    /// Commit diffs by SHA - immutable, cache forever
    commit_diffs: HashMap<String, String>,
    /// Completed job logs by job_id - immutable once completed
    job_logs: HashMap<u64, String>,
}

#[derive(Clone)]
pub struct Client {
    octocrab: Arc<Octocrab>,
    http: reqwest::Client,
    token: String,
    cache: Arc<RwLock<Cache>>,
}

impl Client {
    pub async fn new() -> Result<Self> {
        // Try to get token from: env vars -> .env.local -> gh config
        let token = std::env::var("GITHUB_TOKEN")
            .or_else(|_| std::env::var("GH_TOKEN"))
            .or_else(|_| Self::get_token_from_env_file())
            .or_else(|_| Self::get_gh_config_token())
            .context("No GitHub token found. Set GITHUB_TOKEN env var or login with `gh auth login`")?;

        let octocrab = Octocrab::builder()
            .personal_token(token.clone())
            .build()
            .context("Failed to create GitHub client")?;

        let http = reqwest::Client::new();

        Ok(Self {
            octocrab: Arc::new(octocrab),
            http,
            token,
            cache: Arc::new(RwLock::new(Cache::default())),
        })
    }

    fn get_token_from_env_file() -> Result<String, std::env::VarError> {
        // Try .env.local first, then .env
        let paths = [".env.local", ".env"];

        for path in paths {
            if let Ok(content) = std::fs::read_to_string(path) {
                for line in content.lines() {
                    let line = line.trim();
                    // Skip comments and empty lines
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    // Look for GITHUB_TOKEN= or GH_TOKEN=
                    for prefix in ["GITHUB_TOKEN=", "GH_TOKEN="] {
                        if let Some(token) = line.strip_prefix(prefix) {
                            let token = token.trim().trim_matches('"').trim_matches('\'').to_string();
                            if !token.is_empty() {
                                return Ok(token);
                            }
                        }
                    }
                }
            }
        }

        Err(std::env::VarError::NotPresent)
    }

    fn get_gh_config_token() -> Result<String, std::env::VarError> {
        // Read token directly from gh CLI config file (~/.config/gh/hosts.yml)
        let config_path = dirs::home_dir()
            .ok_or(std::env::VarError::NotPresent)?
            .join(".config/gh/hosts.yml");

        let content = std::fs::read_to_string(config_path)
            .map_err(|_| std::env::VarError::NotPresent)?;

        // Parse YAML manually - look for oauth_token under github.com
        // Format:
        // github.com:
        //     oauth_token: gho_xxxx
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("oauth_token:") {
                if let Some(token) = trimmed.strip_prefix("oauth_token:") {
                    let token = token.trim().to_string();
                    if !token.is_empty() {
                        return Ok(token);
                    }
                }
            }
        }

        Err(std::env::VarError::NotPresent)
    }

    /// Get the current authenticated user
    pub async fn get_current_user(&self) -> Result<String> {
        let url = format!("{}/user", API_BASE);

        let user: serde_json::Value = self.http
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .send()
            .await
            .context("Failed to fetch current user")?
            .json()
            .await
            .context("Failed to parse user response")?;

        user.get("login")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("No login field in user response"))
    }

    pub async fn list_prs(&self, owner: &str, repo: &str) -> Result<Vec<PullRequest>> {
        let page = self
            .octocrab
            .pulls(owner, repo)
            .list()
            .state(octocrab::params::State::Open)
            .per_page(50)
            .send()
            .await
            .context("Failed to fetch PRs")?;

        let prs: Vec<PullRequest> = page
            .items
            .into_iter()
            .map(|pr| PullRequest {
                number: pr.number,
                title: pr.title.unwrap_or_default(),
                body: pr.body.filter(|b| !b.is_empty()),
                state: pr.state.map(|s| format!("{:?}", s).to_lowercase()).unwrap_or_default(),
                user: super::types::User {
                    login: pr.user.map(|u| u.login).unwrap_or_default(),
                    avatar_url: String::new(),
                },
                head: super::types::Branch {
                    ref_name: pr.head.ref_field,
                    sha: pr.head.sha,
                },
                base: super::types::Branch {
                    ref_name: pr.base.ref_field,
                    sha: pr.base.sha,
                },
                draft: pr.draft.unwrap_or(false),
                mergeable: pr.mergeable,
                merged: pr.merged_at.is_some(),
                created_at: pr.created_at.map(|t| t.to_string()).unwrap_or_default(),
                updated_at: pr.updated_at.map(|t| t.to_string()).unwrap_or_default(),
                labels: pr
                    .labels
                    .unwrap_or_default()
                    .into_iter()
                    .map(|l| super::types::Label {
                        name: l.name,
                        color: l.color,
                    })
                    .collect(),
                requested_reviewers: pr
                    .requested_reviewers
                    .unwrap_or_default()
                    .into_iter()
                    .map(|u| super::types::User {
                        login: u.login,
                        avatar_url: String::new(),
                    })
                    .collect(),
                ci_status: None,
            })
            .collect();

        Ok(prs)
    }

    pub async fn get_pr_diff(&self, owner: &str, repo: &str, number: u64) -> Result<String> {
        let url = format!("{}/repos/{}/{}/pulls/{}", API_BASE, owner, repo, number);

        let response = self.http
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(ACCEPT, "application/vnd.github.diff")
            .header(USER_AGENT, "github-tui")
            .send()
            .await
            .context("Failed to fetch PR diff")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to fetch PR diff: {}", response.status()));
        }

        response.text().await.context("Failed to read diff response")
    }

    pub async fn approve_pr(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        let url = format!("{}/repos/{}/{}/pulls/{}/reviews", API_BASE, owner, repo, number);

        let response = self.http
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .header(CONTENT_TYPE, "application/json")
            .json(&serde_json::json!({ "event": "APPROVE" }))
            .send()
            .await
            .context("Failed to approve PR")?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to approve PR: {}", response.status()))
        }
    }

    pub async fn merge_pr(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        let url = format!("{}/repos/{}/{}/pulls/{}/merge", API_BASE, owner, repo, number);

        let response = self.http
            .put(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .header(CONTENT_TYPE, "application/json")
            .json(&serde_json::json!({ "merge_method": "squash" }))
            .send()
            .await
            .context("Failed to merge PR")?;

        if response.status().is_success() {
            Ok(())
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Failed to merge PR: {}", body))
        }
    }

    pub async fn edit_pr_title(&self, owner: &str, repo: &str, number: u64, title: &str) -> Result<()> {
        let url = format!("{}/repos/{}/{}/pulls/{}", API_BASE, owner, repo, number);

        let response = self.http
            .patch(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .header(CONTENT_TYPE, "application/json")
            .json(&serde_json::json!({ "title": title }))
            .send()
            .await
            .context("Failed to edit PR title")?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to edit PR title: {}", response.status()))
        }
    }

    pub async fn edit_pr_body(&self, owner: &str, repo: &str, number: u64, body: &str) -> Result<()> {
        let url = format!("{}/repos/{}/{}/pulls/{}", API_BASE, owner, repo, number);

        let response = self.http
            .patch(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .header(CONTENT_TYPE, "application/json")
            .json(&serde_json::json!({ "body": body }))
            .send()
            .await
            .context("Failed to edit PR description")?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to edit PR description: {}", response.status()))
        }
    }

    pub async fn add_pr_labels(&self, owner: &str, repo: &str, number: u64, labels: &[&str]) -> Result<()> {
        if labels.is_empty() {
            return Ok(());
        }

        // PRs share issue numbers, so use issues endpoint for labels
        let url = format!("{}/repos/{}/{}/issues/{}/labels", API_BASE, owner, repo, number);

        let response = self.http
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .header(CONTENT_TYPE, "application/json")
            .json(&serde_json::json!({ "labels": labels }))
            .send()
            .await
            .context("Failed to add labels")?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to add labels: {}", response.status()))
        }
    }

    pub async fn add_pr_reviewers(&self, owner: &str, repo: &str, number: u64, reviewers: &[&str]) -> Result<()> {
        if reviewers.is_empty() {
            return Ok(());
        }

        let url = format!("{}/repos/{}/{}/pulls/{}/requested_reviewers", API_BASE, owner, repo, number);

        let response = self.http
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .header(CONTENT_TYPE, "application/json")
            .json(&serde_json::json!({ "reviewers": reviewers }))
            .send()
            .await
            .context("Failed to add reviewers")?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to add reviewers: {}", response.status()))
        }
    }

    pub async fn list_runs(&self, owner: &str, repo: &str) -> Result<Vec<WorkflowRun>> {
        let runs = self
            .octocrab
            .workflows(owner, repo)
            .list_all_runs()
            .per_page(30)
            .send()
            .await
            .context("Failed to fetch workflow runs")?;

        Ok(runs.items.into_iter().map(Self::convert_run).collect())
    }

    pub async fn list_runs_for_commit(&self, owner: &str, repo: &str, sha: &str) -> Result<Vec<WorkflowRun>> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs?head_sha={}&per_page=20",
            API_BASE, owner, repo, sha
        );

        let response: WorkflowRunsResponse = self.http
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .send()
            .await
            .context("Failed to fetch runs for commit")?
            .json()
            .await
            .context("Failed to parse runs response")?;

        Ok(response.workflow_runs.into_iter().map(|r| WorkflowRun {
            id: r.id,
            name: r.name.unwrap_or_default(),
            head_branch: r.head_branch,
            head_sha: r.head_sha,
            status: r.status,
            conclusion: r.conclusion,
            run_number: r.run_number,
            event: r.event,
            created_at: r.created_at,
            updated_at: r.updated_at,
            html_url: r.html_url,
        }).collect())
    }

    fn convert_run(run: octocrab::models::workflows::Run) -> WorkflowRun {
        WorkflowRun {
            id: run.id.into_inner(),
            name: run.name,
            head_branch: run.head_branch,
            head_sha: run.head_sha,
            status: run.status,
            conclusion: run.conclusion,
            run_number: run.run_number as u64,
            event: run.event,
            created_at: run.created_at.to_string(),
            updated_at: run.updated_at.to_string(),
            html_url: run.html_url.to_string(),
        }
    }

    pub async fn list_jobs(&self, owner: &str, repo: &str, run_id: u64) -> Result<Vec<Job>> {
        let jobs = self
            .octocrab
            .workflows(owner, repo)
            .list_jobs(run_id.into())
            .per_page(50)
            .send()
            .await
            .context("Failed to fetch jobs")?;

        let job_list: Vec<Job> = jobs
            .items
            .into_iter()
            .map(|job| Job {
                id: job.id.into_inner(),
                run_id: job.run_id.into_inner(),
                name: job.name,
                status: format!("{:?}", job.status).to_lowercase(),
                conclusion: job.conclusion.map(|c| format!("{:?}", c).to_lowercase()),
                started_at: job.started_at.to_string(),
                completed_at: job.completed_at.map(|t| t.to_string()),
                steps: job
                    .steps
                    .into_iter()
                    .map(|s| super::types::Step {
                        name: s.name,
                        status: format!("{:?}", s.status).to_lowercase(),
                        conclusion: s.conclusion.map(|c| format!("{:?}", c).to_lowercase()),
                        number: s.number as u64,
                    })
                    .collect(),
            })
            .collect();

        Ok(job_list)
    }

    pub async fn get_run_logs(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
        job_id: Option<u64>,
    ) -> Result<String> {
        // Check cache first for job logs (completed jobs are immutable)
        if let Some(jid) = job_id {
            let cache = self.cache.read().await;
            if let Some(logs) = cache.job_logs.get(&jid) {
                return Ok(logs.clone());
            }
        }

        // If job_id specified, get job logs, otherwise get run logs
        let url = if let Some(jid) = job_id {
            format!("{}/repos/{}/{}/actions/jobs/{}/logs", API_BASE, owner, repo, jid)
        } else {
            format!("{}/repos/{}/{}/actions/runs/{}/logs", API_BASE, owner, repo, run_id)
        };

        let response = self.http
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status() == 404 {
                    return Ok("Logs not available yet. The run may still be in progress or queued.".to_string());
                }

                if !resp.status().is_success() {
                    return Err(anyhow::anyhow!("Failed to fetch logs: {}", resp.status()));
                }

                let bytes = resp.bytes().await.context("Failed to read logs response")?;

                // The response is a zip file, try to extract it
                let logs = if let Ok(extracted) = Self::extract_logs_from_zip(&bytes) {
                    extracted
                } else {
                    // If not a zip, try as plain text
                    String::from_utf8_lossy(&bytes).to_string()
                };

                // Cache job logs (completed jobs are immutable)
                if let Some(jid) = job_id {
                    let mut cache = self.cache.write().await;
                    cache.job_logs.insert(jid, logs.clone());
                }

                Ok(logs)
            }
            Err(e) => Err(anyhow::anyhow!("Failed to fetch logs: {}", e)),
        }
    }

    fn extract_logs_from_zip(data: &[u8]) -> Result<String> {
        use std::io::Read;

        let cursor = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor).context("Failed to open zip archive")?;

        let mut all_logs = String::new();

        // Sort entries by name for consistent ordering
        let mut names: Vec<String> = (0..archive.len())
            .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
            .collect();
        names.sort();

        for name in names {
            if let Ok(mut file) = archive.by_name(&name) {
                // Add header for each log file
                all_logs.push_str(&format!("\n=== {} ===\n", name));

                let mut contents = String::new();
                if file.read_to_string(&mut contents).is_ok() {
                    all_logs.push_str(&contents);
                }
            }
        }

        if all_logs.is_empty() {
            Err(anyhow::anyhow!("No log files found in archive"))
        } else {
            Ok(all_logs)
        }
    }

    pub async fn rerun_workflow(&self, owner: &str, repo: &str, run_id: u64) -> Result<()> {
        // First try to rerun only failed jobs
        let url_failed = format!(
            "{}/repos/{}/{}/actions/runs/{}/rerun-failed-jobs",
            API_BASE, owner, repo, run_id
        );

        let response = self.http
            .post(&url_failed)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .send()
            .await;

        if let Ok(resp) = response {
            if resp.status().is_success() {
                return Ok(());
            }
        }

        // If rerun-failed-jobs fails, try full rerun
        let url_full = format!(
            "{}/repos/{}/{}/actions/runs/{}/rerun",
            API_BASE, owner, repo, run_id
        );

        let response = self.http
            .post(&url_full)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .send()
            .await
            .context("Failed to rerun workflow")?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to rerun workflow: {}", response.status()))
        }
    }

    pub async fn list_pr_commits(&self, owner: &str, repo: &str, number: u64) -> Result<Vec<Commit>> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}/commits?per_page=100",
            API_BASE, owner, repo, number
        );

        let commits: Vec<CommitResponse> = self.http
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .send()
            .await
            .context("Failed to fetch PR commits")?
            .json()
            .await
            .context("Failed to parse commits response")?;

        Ok(commits.into_iter().map(|c| {
            let author = c.commit.author.as_ref()
                .map(|a| a.name.clone())
                .unwrap_or_else(|| "unknown".to_string());
            let date = c.commit.author.as_ref()
                .map(|a| a.date.clone())
                .unwrap_or_default();
            Commit {
                sha: c.sha,
                message: c.commit.message.lines().next().unwrap_or("").to_string(),
                author,
                date,
            }
        }).collect())
    }

    pub async fn get_commit_diff(&self, owner: &str, repo: &str, sha: &str) -> Result<String> {
        // Check cache first - commit diffs are immutable
        {
            let cache = self.cache.read().await;
            if let Some(diff) = cache.commit_diffs.get(sha) {
                return Ok(diff.clone());
            }
        }

        let url = format!("{}/repos/{}/{}/commits/{}", API_BASE, owner, repo, sha);

        let response = self.http
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(ACCEPT, "application/vnd.github.diff")
            .header(USER_AGENT, "github-tui")
            .send()
            .await
            .context("Failed to fetch commit diff")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to fetch commit diff: {}", response.status()));
        }

        let diff = response.text().await.context("Failed to read commit diff response")?;

        // Cache the result
        {
            let mut cache = self.cache.write().await;
            cache.commit_diffs.insert(sha.to_string(), diff.clone());
        }

        Ok(diff)
    }

    pub async fn list_pr_reviews(&self, owner: &str, repo: &str, number: u64) -> Result<Vec<Review>> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}/reviews",
            API_BASE, owner, repo, number
        );

        let reviews: Vec<ReviewResponse> = self.http
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .send()
            .await
            .context("Failed to fetch PR reviews")?
            .json()
            .await
            .context("Failed to parse reviews response")?;

        Ok(reviews.into_iter().map(|r| Review {
            user: super::types::User {
                login: r.user.login,
                avatar_url: r.user.avatar_url.unwrap_or_default(),
            },
            state: r.state,
            submitted_at: r.submitted_at,
        }).collect())
    }

    /// Find a recently pushed branch without an open PR
    /// Returns the most recently pushed branch by the current user that doesn't have a PR
    pub async fn find_recent_branch_without_pr(
        &self,
        owner: &str,
        repo: &str,
        current_user: &str,
        open_pr_branches: &[String],
    ) -> Result<Option<super::types::RecentBranch>> {
        // Fetch recent events for the repo
        let url = format!("{}/repos/{}/{}/events?per_page=30", API_BASE, owner, repo);

        let events: Vec<EventResponse> = self.http
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "github-tui")
            .send()
            .await
            .context("Failed to fetch events")?
            .json()
            .await
            .context("Failed to parse events response")?;

        // Find push events by the current user to branches without PRs
        let now = chrono::Utc::now();
        let max_age_minutes = 60; // Only show branches pushed in the last hour

        for event in events {
            if event.event_type != "PushEvent" {
                continue;
            }

            // Check if event is from the current user
            if event.actor.login != current_user {
                continue;
            }

            // Extract branch name from ref (refs/heads/branch-name -> branch-name)
            let branch_name = event.payload.ref_field
                .as_ref()
                .and_then(|r| r.strip_prefix("refs/heads/"))
                .map(|s| s.to_string());

            let Some(branch) = branch_name else {
                continue;
            };

            // Skip main/master branches
            if branch == "main" || branch == "master" {
                continue;
            }

            // Check if this branch already has a PR
            if open_pr_branches.contains(&branch) {
                continue;
            }

            // Parse the event time and check if it's recent
            if let Ok(pushed_at) = chrono::DateTime::parse_from_rfc3339(&event.created_at) {
                let age = now.signed_duration_since(pushed_at.with_timezone(&chrono::Utc));
                let minutes_ago = age.num_minutes() as u64;

                if minutes_ago <= max_age_minutes {
                    return Ok(Some(super::types::RecentBranch {
                        name: branch,
                        pushed_at: event.created_at,
                        minutes_ago,
                    }));
                }
            }
        }

        Ok(None)
    }
}

// Response types for API calls

#[derive(serde::Deserialize)]
struct WorkflowRunsResponse {
    workflow_runs: Vec<WorkflowRunJson>,
}

#[derive(serde::Deserialize)]
struct WorkflowRunJson {
    id: u64,
    name: Option<String>,
    head_branch: String,
    head_sha: String,
    status: String,
    conclusion: Option<String>,
    run_number: u64,
    event: String,
    created_at: String,
    updated_at: String,
    html_url: String,
}

#[derive(serde::Deserialize)]
struct CommitResponse {
    sha: String,
    commit: CommitData,
}

#[derive(serde::Deserialize)]
struct CommitData {
    message: String,
    author: Option<CommitAuthor>,
}

#[derive(serde::Deserialize, Clone)]
struct CommitAuthor {
    name: String,
    #[serde(default)]
    date: String,
}

#[derive(serde::Deserialize)]
struct ReviewResponse {
    user: ReviewUser,
    state: String,
    submitted_at: Option<String>,
}

#[derive(serde::Deserialize)]
struct ReviewUser {
    login: String,
    avatar_url: Option<String>,
}

#[derive(serde::Deserialize)]
struct EventResponse {
    #[serde(rename = "type")]
    event_type: String,
    actor: EventActor,
    created_at: String,
    #[serde(default)]
    payload: EventPayload,
}

#[derive(serde::Deserialize)]
struct EventActor {
    login: String,
}

#[derive(serde::Deserialize, Default)]
struct EventPayload {
    #[serde(rename = "ref")]
    ref_field: Option<String>,
}
