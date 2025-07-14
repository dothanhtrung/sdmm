//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

pub mod item;
pub mod tag;

use crate::config::DBConfig;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::SqlitePool;
use std::str::FromStr;

pub struct DBPool {
    pub sqlite_pool: SqlitePool,
}

impl DBPool {
    pub async fn init(config: &DBConfig) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::from_str(&config.sqlite.db_path)?
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .create_if_missing(true);
        let sqlite_pool = SqlitePool::connect_with(opts).await?;
        sqlx::migrate!("./migrations").run(&sqlite_pool).await?;

        Ok(Self { sqlite_pool })
    }
}
