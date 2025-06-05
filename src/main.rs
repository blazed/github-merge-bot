use anyhow::Result;
use axum::{extract::State, http::StatusCode, response::Json, routing::post, Router};
use chrono::Utc;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};

mod commands;
mod config;
mod database;
mod github;
mod webhook;

use commands::CommandProcessor;
use config::Config;
use database::Database;
use github::GitHubClient;
use webhook::WebhookHandler;

// Import types from lib
use github_merge_bot::{PullRequest, Repository, TryMergeJob};

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Config,
    pub db: Database,
    pub github: GitHubClient,
    pub webhook_handler: WebhookHandler,
    pub command_processor: Arc<Mutex<CommandProcessor>>,
    pub active_jobs: Arc<RwLock<HashMap<String, TryMergeJob>>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::init();

    let config = Config::load()?;
    let db = Database::new(&config.database_url).await?;
    let github = GitHubClient::new(&config.github_token);
    let webhook_handler = WebhookHandler::new(&config.webhook_secret);
    let command_processor = Arc::new(Mutex::new(CommandProcessor::new()));
    let active_jobs = Arc::new(RwLock::new(HashMap::new()));

    // Initialize database
    db.migrate().await?;

    let state = AppState {
        config: config.clone(),
        db,
        github,
        webhook_handler,
        command_processor,
        active_jobs,
    };

    let app = Router::new()
        .route("/webhook", post(handle_webhook))
        .route("/health", axum::routing::get(health_check))
        .with_state(Arc::new(state))
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&config.bind_address).await?;
    info!("Server starting on {}", config.bind_address);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_webhook(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Result<StatusCode, StatusCode> {
    let event_type = headers
        .get("X-GitHub-Event")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    if !state
        .webhook_handler
        .verify_signature(&headers, &body)
        .await
    {
        warn!("Invalid webhook signature");
        return Err(StatusCode::UNAUTHORIZED);
    }

    let payload: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

    tokio::spawn(async move {
        if let Err(e) = process_webhook_event(&state, event_type, payload).await {
            error!("Error processing webhook: {}", e);
        }
    });

    Ok(StatusCode::OK)
}

async fn process_webhook_event(
    state: &AppState,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<()> {
    match event_type {
        "issue_comment" => {
            if let Some(comment_body) = payload["comment"]["body"].as_str() {
                if let Some(pr_number) = payload["issue"]["number"].as_i64() {
                    let repo = Repository {
                        id: payload["repository"]["id"].as_i64().unwrap_or(0),
                        name: payload["repository"]["name"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        full_name: payload["repository"]["full_name"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        owner: payload["repository"]["owner"]["login"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        default_branch: payload["repository"]["default_branch"]
                            .as_str()
                            .unwrap_or("main")
                            .to_string(),
                    };

                    process_comment_command(state, &repo, pr_number as i32, comment_body).await?;
                }
            }
        }
        "pull_request" => {
            if let Some(action) = payload["action"].as_str() {
                match action {
                    "opened" | "synchronize" | "reopened" => {
                        // Handle PR updates
                        info!("PR {} {}", payload["pull_request"]["number"], action);
                    }
                    _ => {}
                }
            }
        }
        _ => {
            info!("Unhandled webhook event: {}", event_type);
        }
    }

    Ok(())
}

async fn process_comment_command(
    state: &AppState,
    repo: &Repository,
    pr_number: i32,
    comment_body: &str,
) -> Result<()> {
    let processor = state.command_processor.lock().await;

    if let Some(command) = processor.parse_command(comment_body) {
        info!("Processing command: {:?} for PR {}", command, pr_number);

        match command.as_str() {
            "try" => {
                execute_try_merge(state, repo, pr_number, "automation/bot/try").await?;
            }
            "try-merge" => {
                execute_try_merge(state, repo, pr_number, "automation/bot/try-merge").await?;
            }
            _ => {
                warn!("Unknown command: {}", command);
            }
        }
    }

    Ok(())
}

async fn execute_try_merge(
    state: &AppState,
    repo: &Repository,
    pr_number: i32,
    branch_prefix: &str,
) -> Result<()> {
    let job_key = format!("{}#{}", repo.full_name, pr_number);

    // Check if job is already running
    {
        let active_jobs = state.active_jobs.read().await;
        if active_jobs.contains_key(&job_key) {
            info!("Job already running for {}", job_key);
            return Ok(());
        }
    }

    // Create new job
    let job = TryMergeJob {
        id: uuid::Uuid::new_v4(),
        repository_id: repo.id,
        pr_number,
        branch_name: format!("{}/{}", branch_prefix, pr_number),
        status: "running".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        error_message: None,
    };

    // Store job in active jobs
    {
        let mut active_jobs = state.active_jobs.write().await;
        active_jobs.insert(job_key.clone(), job.clone());
    }

    // Store job in database
    state.db.create_try_merge_job(&job).await?;

    // Execute merge operation
    let result = perform_try_merge(state, repo, pr_number, &job.branch_name).await;

    // Update job status
    let mut updated_job = job.clone();
    match result {
        Ok(_) => {
            updated_job.status = "completed".to_string();
            info!("Try merge completed successfully for {}", job_key);
        }
        Err(e) => {
            updated_job.status = "failed".to_string();
            updated_job.error_message = Some(e.to_string());
            error!("Try merge failed for {}: {}", job_key, e);
        }
    }

    updated_job.updated_at = Utc::now();
    state.db.update_try_merge_job(&updated_job).await?;

    // Remove from active jobs
    {
        let mut active_jobs = state.active_jobs.write().await;
        active_jobs.remove(&job_key);
    }

    Ok(())
}

async fn perform_try_merge(
    state: &AppState,
    repo: &Repository,
    pr_number: i32,
    branch_name: &str,
) -> Result<()> {
    // Get PR details
    let pr = state
        .github
        .get_pull_request(&repo.full_name, pr_number)
        .await?;

    // Create or update the try branch
    state
        .github
        .create_try_branch(
            &repo.full_name,
            &pr.head_branch,
            &repo.default_branch,
            branch_name,
        )
        .await?;

    // Wait for CI to complete (simplified)
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Check if merge is successful
    let status = state
        .github
        .get_branch_status(&repo.full_name, branch_name)
        .await?;

    if status == "success" {
        info!("Try merge successful for {}/{}", repo.full_name, pr_number);
    } else {
        anyhow::bail!("Try merge failed with status: {}", status);
    }

    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": Utc::now()
    }))
}
