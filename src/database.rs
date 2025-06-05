// database.rs
use anyhow::Result;
use github_merge_bot::TryMergeJob;
use sqlx::{PgPool, Row};

#[derive(Debug, Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Database { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS repositories (
                id BIGINT PRIMARY KEY,
                name TEXT NOT NULL,
                full_name TEXT NOT NULL UNIQUE,
                owner TEXT NOT NULL,
                default_branch TEXT NOT NULL DEFAULT 'main',
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS try_merge_jobs (
                id UUID PRIMARY KEY,
                repository_id BIGINT NOT NULL,
                pr_number INTEGER NOT NULL,
                branch_name TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW(),
                error_message TEXT,
                CONSTRAINT fk_repository FOREIGN KEY (repository_id) REFERENCES repositories(id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_try_merge_jobs_repo_pr 
            ON try_merge_jobs(repository_id, pr_number)
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_try_merge_job(&self, job: &TryMergeJob) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO try_merge_jobs 
            (id, repository_id, pr_number, branch_name, status, created_at, updated_at, error_message)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(&job.id)
        .bind(&job.repository_id)
        .bind(&job.pr_number)
        .bind(&job.branch_name)
        .bind(&job.status)
        .bind(&job.created_at)
        .bind(&job.updated_at)
        .bind(&job.error_message)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_try_merge_job(&self, job: &TryMergeJob) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE try_merge_jobs 
            SET status = $2, updated_at = $3, error_message = $4
            WHERE id = $1
            "#,
        )
        .bind(&job.id)
        .bind(&job.status)
        .bind(&job.updated_at)
        .bind(&job.error_message)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_active_jobs(&self, repository_id: i64) -> Result<Vec<TryMergeJob>> {
        let rows = sqlx::query(
            r#"
            SELECT id, repository_id, pr_number, branch_name, status, 
                   created_at, updated_at, error_message
            FROM try_merge_jobs 
            WHERE repository_id = $1 AND status IN ('pending', 'running')
            ORDER BY created_at DESC
            "#,
        )
        .bind(repository_id)
        .fetch_all(&self.pool)
        .await?;

        let jobs = rows
            .into_iter()
            .map(|row| TryMergeJob {
                id: row.get("id"),
                repository_id: row.get("repository_id"),
                pr_number: row.get("pr_number"),
                branch_name: row.get("branch_name"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                error_message: row.get("error_message"),
            })
            .collect();

        Ok(jobs)
    }
}
