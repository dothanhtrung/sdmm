
use sqlx::{Error, SqlitePool};
use std::time::SystemTime;
use serde::Serialize;

#[derive(Serialize)]
#[repr(i64)]
pub enum JobState {
    Running,
    Succeed,
    Failed,
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
    .await?.id;
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
