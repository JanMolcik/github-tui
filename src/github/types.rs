use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub user: User,
    pub head: Branch,
    pub base: Branch,
    pub draft: bool,
    pub mergeable: Option<bool>,
    pub merged: bool,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub requested_reviewers: Vec<User>,
    #[serde(default)]
    pub ci_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub login: String,
    #[serde(default)]
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    #[serde(default)]
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: u64,
    pub name: String,
    pub head_branch: String,
    pub head_sha: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub run_number: u64,
    pub event: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub html_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub run_id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    #[serde(default)]
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

impl Commit {
    pub fn short_sha(&self) -> &str {
        if self.sha.len() >= 7 {
            &self.sha[..7]
        } else {
            &self.sha
        }
    }

    pub fn first_line(&self) -> &str {
        self.message.lines().next().unwrap_or(&self.message)
    }
}

impl PullRequest {
    pub fn status_icon(&self) -> &'static str {
        if self.merged {
            "⊗"  // Merged
        } else if self.state == "closed" {
            "✗"  // Closed
        } else if self.draft {
            "◯"  // Draft
        } else {
            "◉"  // Open
        }
    }

    pub fn ci_icon(&self) -> &'static str {
        match self.ci_status.as_deref() {
            Some("success") => "✓",
            Some("failure") => "✗",
            Some("pending") => "◷",
            Some("error") => "⚠",
            _ => "○",
        }
    }
}

impl WorkflowRun {
    pub fn status_icon(&self) -> &'static str {
        match self.conclusion.as_deref() {
            Some("success") => "✓",
            Some("failure") => "✗",
            Some("cancelled") => "⊘",
            Some("skipped") => "⊘",
            _ => match self.status.as_str() {
                "in_progress" => "◷",
                "queued" => "◯",
                _ => "○",
            },
        }
    }

    pub fn time_ago(&self) -> String {
        // Simplified time ago - in a real app, parse and calculate
        self.updated_at.clone()
    }
}

impl Job {
    pub fn status_icon(&self) -> &'static str {
        match self.conclusion.as_deref() {
            Some("success") => "✓",
            Some("failure") => "✗",
            Some("cancelled") => "⊘",
            Some("skipped") => "⊘",
            _ => match self.status.as_str() {
                "in_progress" => "◷",
                "queued" => "◯",
                _ => "○",
            },
        }
    }

    pub fn duration(&self) -> String {
        if self.completed_at.is_some() {
            "completed".to_string()
        } else if !self.started_at.is_empty() {
            "running...".to_string()
        } else {
            "-".to_string()
        }
    }
}
