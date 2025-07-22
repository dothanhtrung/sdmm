use sqlx::sqlite::SqliteQueryResult;
use sqlx::{Error, SqlitePool};
use std::time::SystemTime;

#[repr(i64)]
pub enum JobState {
    Running,
    Paused,
    Succeed,
    Failed,
}

pub async fn add_job(pool: &SqlitePool, title: &str, desc: &str) -> Result<SqliteQueryResult, Error> {
    sqlx::query!(
        r#"INSERT INTO job (title, desc, state) VALUES (?, ?, ?)"#,
        title,
        desc,
        JobState::Running as i64
    )
    .execute(pool)
    .await
}

pub async fn finish_job(pool: &SqlitePool, id: i64, desc: &str, state: JobState) -> Result<(), anyhow::Error> {
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
