use anyhow::{Context, Result};
use octocrab::Octocrab;
use std::sync::Arc;

use super::types::{Job, PullRequest, WorkflowRun};

#[derive(Clone)]
pub struct Client {
    octocrab: Arc<Octocrab>,
}

impl Client {
    pub async fn new() -> Result<Self> {
        // Try to get token from environment or gh CLI
        let token = std::env::var("GITHUB_TOKEN")
            .or_else(|_| std::env::var("GH_TOKEN"))
            .or_else(|_| Self::get_gh_token())
            .context("No GitHub token found. Set GITHUB_TOKEN env var or login with `gh auth login`")?;

        let octocrab = Octocrab::builder()
            .personal_token(token)
            .build()
            .context("Failed to create GitHub client")?;

        Ok(Self { octocrab: Arc::new(octocrab) })
    }

    fn get_gh_token() -> Result<String, std::env::VarError> {
        let output = std::process::Command::new("gh")
            .args(["auth", "token"])
            .output()
            .map_err(|_| std::env::VarError::NotPresent)?;

        if output.status.success() {
            let token = String::from_utf8(output.stdout)
                .map_err(|_| std::env::VarError::NotPresent)?
                .trim()
                .to_string();
            if token.is_empty() {
                Err(std::env::VarError::NotPresent)
            } else {
                Ok(token)
            }
        } else {
            Err(std::env::VarError::NotPresent)
        }
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
        // Use gh CLI for diff - it's faster and handles large diffs better
        let output = tokio::process::Command::new("gh")
            .args([
                "pr",
                "diff",
                &number.to_string(),
                "--repo",
                &format!("{}/{}", owner, repo),
            ])
            .output()
            .await
            .context("Failed to run gh pr diff")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow::anyhow!(
                "gh pr diff failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    pub async fn approve_pr(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        let output = tokio::process::Command::new("gh")
            .args([
                "pr",
                "review",
                &number.to_string(),
                "--repo",
                &format!("{}/{}", owner, repo),
                "--approve",
            ])
            .output()
            .await
            .context("Failed to run gh pr review")?;

        if output.status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "gh pr review failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    pub async fn merge_pr(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        let output = tokio::process::Command::new("gh")
            .args([
                "pr",
                "merge",
                &number.to_string(),
                "--repo",
                &format!("{}/{}", owner, repo),
                "--squash",
            ])
            .output()
            .await
            .context("Failed to run gh pr merge")?;

        if output.status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "gh pr merge failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    pub async fn edit_pr_title(&self, owner: &str, repo: &str, number: u64, title: &str) -> Result<()> {
        let output = tokio::process::Command::new("gh")
            .args([
                "pr", "edit",
                &number.to_string(),
                "--repo", &format!("{}/{}", owner, repo),
                "--title", title,
            ])
            .output()
            .await
            .context("Failed to run gh pr edit")?;

        if output.status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "gh pr edit failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    pub async fn add_pr_labels(&self, owner: &str, repo: &str, number: u64, labels: &[&str]) -> Result<()> {
        if labels.is_empty() {
            return Ok(());
        }

        let mut args = vec![
            "pr".to_string(), "edit".to_string(),
            number.to_string(),
            "--repo".to_string(), format!("{}/{}", owner, repo),
        ];

        for label in labels {
            args.push("--add-label".to_string());
            args.push(label.to_string());
        }

        let output = tokio::process::Command::new("gh")
            .args(&args)
            .output()
            .await
            .context("Failed to run gh pr edit")?;

        if output.status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "gh pr edit failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    pub async fn add_pr_reviewers(&self, owner: &str, repo: &str, number: u64, reviewers: &[&str]) -> Result<()> {
        if reviewers.is_empty() {
            return Ok(());
        }

        let mut args = vec![
            "pr".to_string(), "edit".to_string(),
            number.to_string(),
            "--repo".to_string(), format!("{}/{}", owner, repo),
        ];

        for reviewer in reviewers {
            args.push("--add-reviewer".to_string());
            args.push(reviewer.to_string());
        }

        let output = tokio::process::Command::new("gh")
            .args(&args)
            .output()
            .await
            .context("Failed to run gh pr edit")?;

        if output.status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "gh pr edit failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    pub async fn open_pr_in_browser(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        let output = tokio::process::Command::new("gh")
            .args([
                "pr", "view",
                &number.to_string(),
                "--repo", &format!("{}/{}", owner, repo),
                "--web",
            ])
            .output()
            .await
            .context("Failed to run gh pr view --web")?;

        if output.status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "gh pr view --web failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
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
        // Use gh CLI to get runs for a specific commit - it's more reliable
        let output = tokio::process::Command::new("gh")
            .args([
                "run",
                "list",
                "--repo",
                &format!("{}/{}", owner, repo),
                "--commit",
                sha,
                "--json",
                "databaseId,name,headBranch,headSha,status,conclusion,number,event,createdAt,updatedAt,url",
                "--limit",
                "20",
            ])
            .output()
            .await
            .context("Failed to run gh run list")?;

        if output.status.success() {
            let json_str = String::from_utf8_lossy(&output.stdout);
            let runs: Vec<GhRunJson> = serde_json::from_str(&json_str)
                .unwrap_or_default();

            Ok(runs.into_iter().map(|r| WorkflowRun {
                id: r.database_id,
                name: r.name,
                head_branch: r.head_branch,
                head_sha: r.head_sha,
                status: r.status,
                conclusion: r.conclusion,
                run_number: r.number,
                event: r.event,
                created_at: r.created_at,
                updated_at: r.updated_at,
                html_url: r.url,
            }).collect())
        } else {
            // Fallback to filtering from all runs
            let all_runs = self.list_runs(owner, repo).await?;
            Ok(all_runs.into_iter().filter(|r| r.head_sha.starts_with(sha) || sha.starts_with(&r.head_sha)).collect())
        }
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
        let repo_arg = format!("{}/{}", owner, repo);
        let run_id_str = run_id.to_string();

        // Try --log first (works for completed runs)
        let mut args: Vec<&str> = vec![
            "run", "view",
            &run_id_str,
            "--repo", &repo_arg,
            "--log",
        ];

        let job_id_str = job_id.map(|jid| jid.to_string());
        if let Some(ref jid_str) = job_id_str {
            args.push("--job");
            args.push(jid_str);
        }

        let output = tokio::process::Command::new("gh")
            .args(&args)
            .output()
            .await
            .context("Failed to run gh run view --log")?;

        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        // Try --log-failed for runs that have failures
        let args_failed: Vec<&str> = vec![
            "run", "view",
            &run_id_str,
            "--repo", &repo_arg,
            "--log-failed",
        ];

        let output = tokio::process::Command::new("gh")
            .args(&args_failed)
            .output()
            .await
            .context("Failed to run gh run view --log-failed")?;

        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        // For in-progress runs, logs may not be available yet
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("in progress") || stderr.contains("queued") {
            return Ok("Run is still in progress. Logs will be available when the run completes or a job finishes.".to_string());
        }

        Err(anyhow::anyhow!(
            "gh run view failed: {}",
            stderr
        ))
    }

    pub async fn rerun_workflow(&self, owner: &str, repo: &str, run_id: u64) -> Result<()> {
        // First try to rerun only failed jobs
        let output = tokio::process::Command::new("gh")
            .args([
                "run",
                "rerun",
                &run_id.to_string(),
                "--failed",
                "--repo",
                &format!("{}/{}", owner, repo),
            ])
            .output()
            .await
            .context("Failed to run gh run rerun")?;

        if output.status.success() {
            Ok(())
        } else {
            // If --failed doesn't work (e.g., no failed jobs), try full rerun
            let output = tokio::process::Command::new("gh")
                .args([
                    "run",
                    "rerun",
                    &run_id.to_string(),
                    "--repo",
                    &format!("{}/{}", owner, repo),
                ])
                .output()
                .await
                .context("Failed to run gh run rerun")?;

            if output.status.success() {
                Ok(())
            } else {
                Err(anyhow::anyhow!(
                    "gh run rerun failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ))
            }
        }
    }
}

// JSON structure for gh run list output
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GhRunJson {
    database_id: u64,
    name: String,
    head_branch: String,
    head_sha: String,
    status: String,
    conclusion: Option<String>,
    number: u64,
    event: String,
    created_at: String,
    updated_at: String,
    url: String,
}
