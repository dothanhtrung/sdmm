//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

mod config;
mod item;
mod maintenance;
mod tag;

use crate::civitai::{calculate_blake3, CivitaiFileMetadata};
use crate::db::item::insert_or_update;
use crate::db::tag::add_tag_from_model_info;
use crate::db::DBPool;
use actix_web::web;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tokio::fs;
use tracing::error;

pub const TRASH_DIR: &str = ".trash";

pub fn scope_config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .configure(maintenance::scope)
            .configure(item::scope)
            .configure(tag::scope)
            .configure(config::scope),
    );
}

#[derive(Deserialize, Default)]
pub struct SearchQuery {
    page: Option<i64>,
    count: Option<i64>,
    pub(crate) search: Option<String>,
    tag_only: Option<bool>,
    id: Option<i64>,
}

#[derive(Deserialize)]
struct DeleteRequest {
    #[serde(rename = "id")]
    ids: Vec<i64>,
}

#[derive(Serialize, Default)]
struct CommonResponse {
    msg: String,
    err: Option<String>,
}

fn get_relative_path(base_path: &str, path: &Path) -> Result<String, anyhow::Error> {
    let base = PathBuf::from(base_path);
    let path = path.strip_prefix(&base)?;
    Ok(path.to_str().unwrap_or_default().to_string())
}

async fn save_model_info(db_pool: &DBPool, path: &Path, label: &str, relative_path: &str) {
    let mut item_json_file = PathBuf::from(path);
    item_json_file.set_extension("json");
    let mut model_json_file = PathBuf::from(path);
    model_json_file.set_extension("model.json");
    let item_info = fs::read_to_string(&item_json_file).await.unwrap_or_default();
    let model_info = fs::read_to_string(&model_json_file).await.unwrap_or_default();

    let item_parsed: Value = serde_json::from_str(&item_info).unwrap_or_default();
    let model_parsed: Value = serde_json::from_str(&model_info).unwrap_or_default();

    let base_model = item_parsed["baseModel"].as_str().unwrap_or_default();

    let mut blake3 = item_parsed["files"][0]["hashes"]["BLAKE3"]
        .as_str()
        .unwrap_or_default()
        .to_string()
        .to_lowercase();
    let mut file_metadata =
        serde_json::from_value::<CivitaiFileMetadata>(item_parsed["files"][0]["metadata"].clone()).unwrap_or_default();
    if let Some(files) = item_parsed["files"].as_array() {
        // If there are more than 1 file, find the metadata by hash
        if files.len() > 1 {
            blake3 = calculate_blake3(path).unwrap_or_default().to_lowercase();
            for file in files.iter() {
                let hash = file["hashes"]["BLAKE3"].as_str().unwrap_or_default().to_lowercase();
                if blake3 == hash {
                    file_metadata =
                        serde_json::from_value::<CivitaiFileMetadata>(file["metadata"].clone()).unwrap_or_default();
                }
            }
        }
    }
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .to_string();

    // Read file metadata on disk
    let mut modified_time = 0;
    if let Ok(local_metadata) = fs::metadata(path).await {
        if let Ok(modified) = local_metadata.modified() {
            modified_time = modified.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        }
    }

    match insert_or_update(
        &db_pool.sqlite_pool,
        Some(name.as_str()),
        relative_path,
        label,
        blake3.as_str(),
        modified_time as i64,
    )
    .await
    {
        Ok(id) => {
            let tags = vec![base_model.to_string()];
            if let Err(e) = add_tag_from_model_info(&db_pool.sqlite_pool, id, &tags, &model_parsed, &file_metadata).await
            {
                error!("Failed to insert tag: {}", e);
            }
        }
        Err(e) => error!("Failed to insert item: {}", e),
    }
}
