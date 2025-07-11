use crate::api::{CommonResponse, DeleteRequest, SearchQuery, TRASH_DIR};
use crate::civitai::{download_file, file_type, get_extension_from_url, get_model_info, FileType, PREVIEW_EXT};
use crate::config::Config;
use crate::db::tag::{update_item_note, update_tag_item, TagCount};
use crate::db::DBPool;
use crate::{api, db, ConfigData, BASE_PATH_PREFIX};
use actix_web::web::Data;
use actix_web::{get, post, rt, web, Responder};
use actix_web_lab::extract::Query;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::max;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{error, info};

pub fn scope(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/item")
            .service(get_item)
            .service(saved_location)
            .service(civitai_download)
            .service(delete)
            .service(update),
    );
}

#[derive(Serialize)]
struct SearchResponse {
    items: Vec<ModelInfo>,
    total: i64,
    tags: Vec<TagCount>,
    err: Option<String>,
}

#[derive(Serialize, Default)]
struct ModelInfo {
    id: i64,
    name: String,
    path: String,
    preview: String,
    video_preview: Option<String>,
    info: String,
    note: String,
}

#[derive(Deserialize)]
struct ItemUpdate {
    item_id: i64,
    tags: String,
    note: String,
}

#[derive(Deserialize)]
struct SavedLocationQuery {
    model_type: String,
    blake3: Option<String>,
}

#[derive(Serialize, Default)]
struct SavedLocationResponse {
    saved_location: String,
    is_downloaded: bool,
}

#[derive(Deserialize)]
struct CivitaiDownloadQuery {
    model_type: Option<String>,
    url: String,
    name: String,
    blake3: String,
    dest: String,
}

#[get("")]
async fn get_item(config: Data<ConfigData>, db_pool: Data<DBPool>, query_params: Query<SearchQuery>) -> impl Responder {
    let config = config.config.read().await;
    let page = max(1, query_params.page.unwrap_or(1)) - 1;
    let limit = max(0, query_params.count.unwrap_or(config.api.per_page as i64));
    let offset = page * limit;
    let mut ret = Vec::new();
    let mut err = None;

    let (items, total) = if let Some(item_id) = query_params.id {
        match db::item::get_by_id(&db_pool.sqlite_pool, item_id).await {
            Ok(item) => (vec![item], 1),
            Err(e) => {
                err = Some(format!("{}", e));
                (Vec::new(), 0)
            }
        }
    } else if let Some(search_string) = &query_params.search {
        let tag_only = query_params.tag_only.unwrap_or(false);
        match db::item::search(&db_pool.sqlite_pool, search_string, limit, offset, tag_only).await {
            Ok((i, t)) => (i, t),
            Err(e) => {
                err = Some(format!("{}", e));
                (Vec::new(), 0)
            }
        }
    } else {
        match db::item::get(&db_pool.sqlite_pool, limit, offset).await {
            Ok((i, t)) => (i, t),
            Err(e) => {
                err = Some(format!("{}", e));
                (Vec::new(), 0)
            }
        }
    };

    let mut item_ids = HashSet::new();
    for item in items {
        let (model_url, json_url, preview_url) = get_abs_path(&config, &item.base_label, &item.path);

        let mut video_preview = None;

        let info = fs::read_to_string(&json_url).await.unwrap_or_default();
        let v: Value = serde_json::from_str(info.as_str()).unwrap_or_default();
        if let Some(url) = v["images"][0]["url"].as_str() {
            if let Some(ext) = get_extension_from_url(url) {
                let mut abs_preview = PathBuf::from(&model_url);
                abs_preview.set_extension(&ext);
                if file_type(&abs_preview).await == FileType::Video {
                    let mut video_preview_path = PathBuf::from(&preview_url);
                    video_preview_path.set_extension(&ext);
                    if let Some(str_path) = video_preview_path.to_str() {
                        video_preview = Some(str_path.to_string());
                    }
                }
            }
        }

        item_ids.insert(item.id);

        ret.push(ModelInfo {
            id: item.id,
            name: item.name.unwrap_or_default(),
            path: model_url,
            preview: preview_url,
            video_preview,
            info,
            note: item.note.clone(),
        })
    }

    let tags = if item_ids.is_empty() {
        Vec::new()
    } else {
        db::tag::list_tags(&db_pool.sqlite_pool, item_ids)
            .await
            .unwrap_or_else(|e| {
                error!("Failed to list tags: {e}");
                Vec::new()
            })
    };

    web::Json(SearchResponse {
        items: ret,
        total,
        tags,
        err,
    })
}

#[get("saved_location")]
async fn saved_location(
    config: Data<ConfigData>,
    db_pool: Data<DBPool>,
    query_params: Query<SavedLocationQuery>,
) -> impl Responder {
    let config = config.config.read().await;

    if let Some(blake3) = query_params.blake3.as_ref() {
        if let Ok(item) = db::item::get_by_hash(&db_pool.sqlite_pool, blake3.to_lowercase().as_str()).await {
            let (path, _, _) = get_abs_path(&config, &item.base_label, &item.path);
            let path = PathBuf::from(path);
            let path = path
                .parent()
                .unwrap_or(Path::new("."))
                .to_str()
                .unwrap_or_default()
                .to_string();
            return web::Json(SavedLocationResponse {
                saved_location: path,
                is_downloaded: true,
            });
        }
    }

    let model_type = query_params.model_type.to_lowercase();

    if let Some(path) = config.civitai.saved_location.get(&model_type) {
        return web::Json(SavedLocationResponse {
            saved_location: path.clone(),
            ..Default::default()
        });
    }

    let mut base_path = String::from("/");
    for (_, path) in config.model_paths.iter() {
        base_path = path.clone();
    }
    web::Json(SavedLocationResponse {
        saved_location: guess_saved_location(base_path.as_str(), &model_type),
        ..Default::default()
    })
}

#[get("civitai_download")]
async fn civitai_download(
    db_pool: Data<DBPool>,
    config_data: Data<ConfigData>,
    params: Query<CivitaiDownloadQuery>,
) -> impl Responder {
    let mut config = config_data.config.write().await.clone();
    let mut path = PathBuf::from(&params.dest);

    if let Err(e) = fs::create_dir_all(&path).await {
        return web::Json(CommonResponse {
            err: Some(format!("Failed to create {path:?}: {e}")),
            ..Default::default()
        });
    }

    path = path.join(&params.name);
    let mut is_inside_base_path = false;
    for (_, base_path) in config.model_paths.iter() {
        let parent = PathBuf::from(base_path);
        if path.starts_with(parent) {
            is_inside_base_path = true;
            break;
        }
    }

    if !is_inside_base_path {
        error!("Destination path {} must be inside base path", path.display());
        return web::Json(CommonResponse {
            err: Some("Destination path must be inside base path".to_string()),
            ..Default::default()
        });
    }

    if let Some(model_type) = params.model_type.as_ref() {
        let model_type = model_type.to_lowercase();
        config.civitai.saved_location.insert(model_type, params.dest.clone());
        let _ = config.save(&config_data.config_path, true);
    }

    let client = Client::new();
    let mut headers = HeaderMap::new();
    if let Ok(bearer) = HeaderValue::from_str(&format!("Bearer {}", config.civitai.api_key)) {
        headers.insert(AUTHORIZATION, bearer);
    }

    rt::spawn(async move {
        info!("Downloading file {}: {}", params.name, params.url);
        // TODO: Verify checksum of downloaded file
        if let Err(e) = download_file(params.url.as_str(), &path, &client, &headers, &config.model_paths).await {
            error!("Failed to download file: {}", e);
        }

        if let Err(e) = get_model_info(&path, &client, &headers, Some(params.blake3.clone()), &config).await {
            error!("Failed to get model info: {}", e);
        }

        for (label, base_path) in config.model_paths.iter() {
            if path.starts_with(PathBuf::from(base_path)) {
                let relative_path = api::get_relative_path(base_path, &path).unwrap_or_default();
                api::save_model_info(&db_pool, &path, label, relative_path.as_str()).await;
                break;
            }
        }
    });

    web::Json(CommonResponse {
        msg: "Downloading in background".to_string(),
        ..Default::default()
    })
}

#[get("delete")]
async fn delete(config: Data<ConfigData>, db_pool: Data<DBPool>, params: Query<DeleteRequest>) -> impl Responder {
    let config = config.config.read().await;
    for id in params.ids.iter() {
        let Ok((rel_path, label)) = db::item::mark_obsolete(&db_pool.sqlite_pool, *id).await else {
            continue;
        };
        let Some(base_path) = config.model_paths.get(&label) else {
            continue;
        };
        let base_path = PathBuf::from(base_path);
        let model_file = base_path.join(rel_path);
        let trash_dir = base_path.join(TRASH_DIR);

        if let Err(e) = fs::create_dir_all(&trash_dir).await {
            error!("Failed to create {:?}: {}", trash_dir, e);
            return web::Json("");
        }

        if let Ok(files) = list_same_filename(&model_file) {
            if let Err(e) = move_to_dir(&files, &trash_dir).await {
                error!("Failed to move file to trash directory: {}", e);
            }
        }
    }

    web::Json("")
}

#[post("update")]
async fn update(db_pool: Data<DBPool>, data: web::Json<ItemUpdate>) -> impl Responder {
    if let Err(e) = update_tag_item(&db_pool.sqlite_pool, data.item_id, data.tags.as_str()).await {
        error!("Failed to update tag: {}", e);
    }

    if let Err(e) = update_item_note(&db_pool.sqlite_pool, data.item_id, data.note.as_str()).await {
        error!("Failed to update note: {}", e);
    }

    web::Json("")
}

async fn move_to_dir(files: &[PathBuf], dir: &PathBuf) -> anyhow::Result<()> {
    for file in files {
        let file_name = file.file_name().unwrap_or_default();
        if !file_name.is_empty() {
            let dest = dir.join(file_name);
            fs::rename(file, dest).await?;
        }
    }

    Ok(())
}

fn list_same_filename(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !path.is_file() {
        return Ok(vec![]);
    }

    let dir = path.parent().unwrap_or(Path::new("."));
    let stem = path.file_stem().unwrap_or_default(); // "filename"

    let matches = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|p| p.is_file() && p.file_stem() == Some(stem))
        .collect();

    Ok(matches)
}

fn guess_saved_location(base_path: &str, model_type: &str) -> String {
    let mut path = PathBuf::from(base_path);
    if model_type.eq_ignore_ascii_case("LORA") {
        path = path.join("loras");
    } else if model_type.eq_ignore_ascii_case("Hypernetwork") {
        path = path.join("hypernetworks");
    } else if model_type.eq_ignore_ascii_case("Checkpoint") {
        path = path.join("checkpoints");
    } else {
        path = path.join(model_type.to_lowercase());
    }

    path.to_str().unwrap_or_default().to_string()
}

/// Return abs path of (model, json) and http path of preview
fn get_abs_path(config: &Config, label: &str, rel_path: &str) -> (String, String, String) {
    let (mut model, mut json, mut preview) = (String::new(), String::new(), String::new());
    if let Some(base_path) = config.model_paths.get(label) {
        let base_path = PathBuf::from(base_path);
        let model_path = base_path.join(rel_path);
        model = model_path.to_str().unwrap_or_default().to_string();

        let mut json_path = model_path.clone();
        json_path.set_extension("json");
        json = json_path.to_str().unwrap_or_default().to_string();

        let img_path = PathBuf::from(format!("/{}{}", BASE_PATH_PREFIX, label));
        let mut preview_path = img_path.join(rel_path);
        preview_path.set_extension(PREVIEW_EXT);
        preview = preview_path.to_str().unwrap_or_default().to_string();
    }

    (model, json, preview)
}
