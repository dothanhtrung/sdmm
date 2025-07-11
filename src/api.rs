//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

mod config;
mod item;
mod maintenance;
mod tag;

use crate::civitai::{calculate_blake3, CivitaiFileMetadata, CivitaiModel};
use crate::db::item::insert_or_update;
use crate::db::tag::add_tag_from_model_info;
use crate::db::DBPool;
use actix_web::web;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
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
    let mut json_file = PathBuf::from(path);
    json_file.set_extension("json");
    let info = fs::read_to_string(&json_file).await.unwrap_or_default();
    let v: Value = serde_json::from_str(&info).unwrap_or_default();

    let base_model = v["baseModel"].as_str().unwrap_or_default();

    let mut blake3 = v["files"][0]["hashes"]["BLAKE3"]
        .as_str()
        .unwrap_or_default()
        .to_string()
        .to_lowercase();
    let mut file_metadata =
        serde_json::from_value::<CivitaiFileMetadata>(v["files"][0]["metadata"].clone()).unwrap_or_default();
    if let Some(files) = v["files"].as_array() {
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
    let model_info = serde_json::from_value::<CivitaiModel>(v["model"].clone()).unwrap_or_default();
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .to_string();

    match insert_or_update(
        &db_pool.sqlite_pool,
        Some(name.as_str()),
        relative_path,
        label,
        blake3.as_str(),
        &model_info.name,
    )
    .await
    {
        Ok(id) => {
            let tags = vec![base_model.to_string()];
            if let Err(e) = add_tag_from_model_info(&db_pool.sqlite_pool, id, &tags, &model_info, &file_metadata).await
            {
                error!("Failed to insert tag: {}", e);
            }
        }
        Err(e) => error!("Failed to insert item: {}", e),
    }
}
