use serde::Serialize;
use sqlx::{Error, SqlitePool};
use std::time::SystemTime;

#[derive(Serialize)]
#[repr(i64)]
pub enum JobState {
    Running,
    Succeed,
    Failed,
}

pub struct Job {
    pub id: i64,
    pub title: String,
    pub desc: String,
    pub state: i64,
    pub started_at: i64,
    pub stopped_at: Option<i64>,
}

pub async fn add_job(pool: &SqlitePool, title: &str, desc: &str) -> Result<i64, Error> {
    let id = sqlx::query!(
        r#"INSERT INTO job (title, desc, state) VALUES (?, ?, ?)
        RETURNING id"#,
        title,
        desc,
        JobState::Running as i64
    )
    .fetch_one(pool)
    .await?
    .id;
    Ok(id)
}

pub async fn update_job(pool: &SqlitePool, id: i64, desc: &str, state: JobState) -> Result<(), anyhow::Error> {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs() as i64;
    let state = state as i64;
    sqlx::query!(
        "UPDATE job SET desc = ?, state = ?, stopped_at = ? WHERE id = ?",
        desc,
        state,
        now,
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get(pool: &SqlitePool, limit: i64, offset: i64) -> Result<(Vec<Job>, i64), sqlx::Error> {
    let items = sqlx::query_as!(
        Job,
        r#"SELECT * FROM job ORDER BY started_at DESC LIMIT ? OFFSET ?"#,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar!("SELECT count(id) FROM job",)
        .fetch_one(pool)
        .await?;

    Ok((items, total))
}

pub async fn clean(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let count = sqlx::query!(r#"DELETE FROM job WHERE stopped_at IS NOT NULL"#)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(count)
}
