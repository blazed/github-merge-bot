// github.rs
use anyhow::Result;
use github_merge_bot::PullRequest;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct GitHubClient {
    client: Client,
    token: String,
}

#[derive(Debug, Deserialize)]
struct GitHubPR {
    id: i64,
    number: i32,
    title: String,
    head: GitHubBranch,
    base: GitHubBranch,
    state: String,
    mergeable: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct GitHubBranch {
    #[serde(rename = "ref")]
    ref_name: String,
    repo: GitHubRepo,
}

#[derive(Debug, Deserialize)]
struct GitHubRepo {
    id: i64,
    name: String,
    full_name: String,
    owner: GitHubUser,
    default_branch: String,
}

#[derive(Debug, Deserialize)]
struct GitHubUser {
    login: String,
}

impl GitHubClient {
    pub fn new(token: &str) -> Self {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
        );
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static("github-merge-bot/1.0"),
        );
        headers.insert(
            "Accept",
            header::HeaderValue::from_static("application/vnd.github.v3+json"),
        );

        let client = Client::builder().default_headers(headers).build().unwrap();

        Self {
            client,
            token: token.to_string(),
        }
    }

    pub async fn get_pull_request(&self, repo: &str, pr_number: i32) -> Result<PullRequest> {
        let url = format!("https://api.github.com/repos/{}/pulls/{}", repo, pr_number);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get PR: {}", response.status());
        }

        let github_pr: GitHubPR = response.json().await?;

        Ok(PullRequest {
            id: github_pr.id,
            number: github_pr.number,
            title: github_pr.title,
            head_branch: github_pr.head.ref_name,
            base_branch: github_pr.base.ref_name,
            repository: github_merge_bot::Repository {
                id: github_pr.base.repo.id,
                name: github_pr.base.repo.name,
                full_name: github_pr.base.repo.full_name,
                owner: github_pr.base.repo.owner.login,
                default_branch: github_pr.base.repo.default_branch,
            },
            state: github_pr.state,
            mergeable: github_pr.mergeable,
        })
    }

    pub async fn create_try_branch(
        &self,
        repo: &str,
        head_branch: &str,
        base_branch: &str,
        try_branch: &str,
    ) -> Result<()> {
        // Get base branch SHA
        let base_sha = self.get_branch_sha(repo, base_branch).await?;

        // Get head branch SHA
        let head_sha = self.get_branch_sha(repo, head_branch).await?;

        // Delete existing try branch if it exists
        let _ = self.delete_branch(repo, try_branch).await;

        // Create try branch from base
        self.create_branch(repo, try_branch, &base_sha).await?;

        // Merge head into try branch
        self.merge_branch(repo, try_branch, &head_sha).await?;

        Ok(())
    }

    async fn get_branch_sha(&self, repo: &str, branch: &str) -> Result<String> {
        let url = format!("https://api.github.com/repos/{}/branches/{}", repo, branch);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get branch {}: {}", branch, response.status());
        }

        let branch_data: serde_json::Value = response.json().await?;
        let sha = branch_data["commit"]["sha"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No SHA found for branch {}", branch))?;

        Ok(sha.to_string())
    }

    async fn create_branch(&self, repo: &str, branch: &str, sha: &str) -> Result<()> {
        let url = format!("https://api.github.com/repos/{}/git/refs", repo);
        let payload = json!({
            "ref": format!("refs/heads/{}", branch),
            "sha": sha
        });

        let response = self.client.post(&url).json(&payload).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to create branch {}: {}", branch, response.status());
        }

        Ok(())
    }

    async fn merge_branch(&self, repo: &str, target_branch: &str, source_sha: &str) -> Result<()> {
        let url = format!("https://api.github.com/repos/{}/merges", repo);
        let payload = json!({
            "base": target_branch,
            "head": source_sha,
            "commit_message": format!("Try merge into {}", target_branch)
        });

        let response = self.client.post(&url).json(&payload).send().await?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Failed to merge into {}: {}",
                target_branch,
                response.status()
            );
        }

        Ok(())
    }

    async fn delete_branch(&self, repo: &str, branch: &str) -> Result<()> {
        let url = format!(
            "https://api.github.com/repos/{}/git/refs/heads/{}",
            repo, branch
        );
        let response = self.client.delete(&url).send().await?;

        // Don't error if branch doesn't exist
        Ok(())
    }

    pub async fn get_branch_status(&self, repo: &str, branch: &str) -> Result<String> {
        let sha = self.get_branch_sha(repo, branch).await?;
        let url = format!(
            "https://api.github.com/repos/{}/commits/{}/status",
            repo, sha
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Ok("unknown".to_string());
        }

        let status: serde_json::Value = response.json().await?;
        let state = status["state"].as_str().unwrap_or("pending");

        Ok(state.to_string())
    }

    pub async fn comment_on_pr(&self, repo: &str, pr_number: i32, comment: &str) -> Result<()> {
        let url = format!(
            "https://api.github.com/repos/{}/issues/{}/comments",
            repo, pr_number
        );
        let payload = json!({
            "body": comment
        });

        let response = self.client.post(&url).json(&payload).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to comment on PR: {}", response.status());
        }

        Ok(())
    }
}
