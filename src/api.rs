//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use crate::civitai::{
    download_file, file_type, get_extension_from_url, update_model_info, CivitaiFileMetadata, CivitaiModel, FileType,
    PREVIEW_EXT,
};
use crate::config::Config;
use crate::db::item::insert_or_update;
use crate::db::tag::{add_tag_from_model_info, update_item_note, update_tag_item, Tag, TagCount};
use crate::db::{item, tag, DBPool};
use crate::{civitai, ConfigData, BASE_PATH_PREFIX};
use actix_web::web::Data;
use actix_web::{get, post, rt, web, Responder};
use actix_web_lab::extract::Query;
use jwalk::{Parallelism, WalkDir};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::max;
use std::collections::HashSet;

use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{error, info};

const TRASH_DIR: &str = ".trash";

pub fn scope_config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .service(get)
            .service(scan_folder)
            .service(remove_orphan)
            .service(delete)
            .service(empty_trash)
            .service(search)
            .service(update_item)
            .service(list_tags)
            .service(saved_location)
            .service(civitai_download)
            .service(get_tag)
            .service(update_tag)
            .service(delete_tag)
            .service(check_downloaded)
            .service(sync_civitai),
    );
}

#[derive(Serialize)]
struct SearchResponse {
    items: Vec<ModelInfo>,
    total: i64,
    tags: Vec<TagCount>,
    err: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct SearchQuery {
    page: Option<i64>,
    count: Option<i64>,
    pub(crate) search: Option<String>,
    tag_only: Option<bool>,
    id: Option<i64>,
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
struct DeleteRequest {
    #[serde(rename = "id")]
    ids: Vec<i64>,
}

#[derive(Deserialize)]
struct SavedLocationQuery {
    model_type: String,
    blake3: Option<String>,
}

#[derive(Serialize)]
struct SavedLocationResponse {
    saved_location: String,
}

#[derive(Deserialize)]
struct CivitaiDownloadQuery {
    model_type: Option<String>,
    url: String,
    name: String,
    blake3: String,
    dest: String,
}

#[derive(Serialize)]
struct TagResponse {
    tag: Option<Tag>,
    msg: String,
}

#[derive(Serialize, Default)]
struct CommonResponse {
    msg: String,
    err: Option<String>,
}

#[derive(Deserialize)]
struct CheckDownloadedQuery {
    blake3: String,
}

#[get("")]
async fn get(config: Data<ConfigData>, db_pool: Data<DBPool>, query_params: Query<SearchQuery>) -> impl Responder {
    let config = config.config.lock().await;
    let page = max(1, query_params.page.unwrap_or(1)) - 1;
    let limit = max(0, query_params.count.unwrap_or(config.api.per_page as i64));
    let offset = page * limit;
    let mut ret = Vec::new();
    let mut err = None;

    let (items, total) = if let Some(item_id) = query_params.id {
        match item::get_by_id(&db_pool.sqlite_pool, item_id).await {
            Ok(item) => (vec![item], 1),
            Err(e) => {
                err = Some(format!("{}", e));
                (Vec::new(), 0)
            }
        }
    } else if let Some(search_string) = &query_params.search {
        let tag_only = query_params.tag_only.unwrap_or(false);
        match item::search(&db_pool.sqlite_pool, search_string, limit, offset, tag_only).await {
            Ok((i, t)) => (i, t),
            Err(e) => {
                err = Some(format!("{}", e));
                (Vec::new(), 0)
            }
        }
    } else {
        match item::get(&db_pool.sqlite_pool, limit, offset).await {
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
        let mut info = String::new();
        let mut video_preview = None;

        // Query for only one item
        // if query_params.id.is_some() {
        let info_str = fs::read_to_string(&json_url).await.unwrap_or_default();
        let v: Value = serde_json::from_str(info_str.as_str()).unwrap_or_default();
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

        info = info_str;
        // }

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
        tag::list_tags(&db_pool.sqlite_pool, item_ids).await.unwrap_or_default()
    };

    web::Json(SearchResponse {
        items: ret,
        total,
        tags,
        err,
    })
}

#[get("scan_folder")]
async fn scan_folder(config: Data<ConfigData>, db_pool: Data<DBPool>) -> impl Responder {
    rt::spawn(async move {
        scan(config, db_pool).await;
    });
    web::Json("")
}

#[get("remove_orphan")]
async fn remove_orphan(db_pool: Data<DBPool>) -> impl Responder {
    let deleted_items = item::clean(&db_pool.sqlite_pool).await.unwrap_or_default();

    web::Json(format!(
        "{{
        \"deleted_items\": {},
    }}",
        deleted_items,
    ))
}

#[get("sync_civitai")]
async fn sync_civitai(config_data: Data<ConfigData>, db_pool: Data<DBPool>) -> impl Responder {
    let config = config_data.config.lock().await.clone();
    rt::spawn(async move {
        let _ = update_model_info(config).await;
        scan(config_data, db_pool).await;
    });
    web::Json("")
}

#[get("saved_location")]
async fn saved_location(config: Data<ConfigData>, query_params: Query<SavedLocationQuery>) -> impl Responder {
    let config = config.config.lock().await;
    if let Some(path) = config.civitai.saved_location.get(&query_params.model_type) {
        return web::Json(SavedLocationResponse {
            saved_location: path.clone(),
        });
    }

    let mut base_path = String::from("/");
    for (_, path) in config.model_paths.iter() {
        base_path = path.clone();
    }
    web::Json(SavedLocationResponse {
        saved_location: guess_saved_location(base_path.as_str(), &query_params.model_type),
    })
}

#[get("civitai_download")]
async fn civitai_download(
    db_pool: Data<DBPool>,
    config_data: Data<ConfigData>,
    params: Query<CivitaiDownloadQuery>,
) -> impl Responder {
    let mut config = config_data.config.lock().await.clone();
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

    if let Some(model_type) = params.model_type.clone() {
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
        if let Err(e) = download_file(params.url.as_str(), &path, &client, &headers).await {
            error!("Failed to download file: {}", e);
        }

        if let Err(e) =
            civitai::get_model_info(&path, &client, &headers, Some(params.blake3.clone()), &config.civitai).await
        {
            error!("Failed to get model info: {}", e);
        }

        for (label, base_path) in config.model_paths.iter() {
            if path.starts_with(PathBuf::from(base_path)) {
                let relative_path = get_relative_path(base_path, &path).unwrap_or_default();
                save_model_info(&db_pool, &path, label, relative_path.as_str()).await;
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
    let config = config.config.lock().await;
    for id in params.ids.iter() {
        let Ok((rel_path, label)) = item::mark_obsolete(&db_pool.sqlite_pool, *id).await else {
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

#[get("empty_trash")]
async fn empty_trash(config: Data<ConfigData>) -> impl Responder {
    let config = config.config.lock().await;
    for (_, base_path) in config.model_paths.iter() {
        let trash_dir = PathBuf::from(base_path).join(TRASH_DIR);
        if let Err(e) = fs::remove_dir_all(&trash_dir).await {
            error!("Failed to remove trash directory: {}", e);
        }
    }
    web::Json("")
}

#[get("search")]
async fn search() -> impl Responder {
    web::Json("")
}

#[post("update_item")]
async fn update_item(db_pool: Data<DBPool>, data: web::Json<ItemUpdate>) -> impl Responder {
    if let Err(e) = update_tag_item(&db_pool.sqlite_pool, data.item_id, data.tags.as_str()).await {
        error!("Failed to update tag: {}", e);
    }

    if let Err(e) = update_item_note(&db_pool.sqlite_pool, data.item_id, data.note.as_str()).await {
        error!("Failed to update note: {}", e);
    }

    web::Json("")
}

#[get("list_tags")]
async fn list_tags(db_pool: Data<DBPool>) -> impl Responder {
    let list_tags = tag::list_tags(&db_pool.sqlite_pool, HashSet::new())
        .await
        .unwrap_or_default();
    web::Json(list_tags)
}

#[get("tag/{tag}")]
async fn get_tag(db_pool: Data<DBPool>, tag: web::Path<String>) -> impl Responder {
    let res = match tag::get_tag_by_name(&db_pool.sqlite_pool, tag.into_inner().as_str()).await {
        Ok(tag) => TagResponse {
            tag: Some(tag),
            msg: "".to_string(),
        },
        Err(e) => TagResponse {
            tag: None,
            msg: format!("{}", e),
        },
    };
    web::Json(res)
}

#[post("tag")]
async fn update_tag(db_pool: Data<DBPool>, data: web::Json<Tag>) -> impl Responder {
    if let Err(e) = tag::update_tag(&db_pool.sqlite_pool, &data.into_inner()).await {
        return web::Json(format!("{e}"));
    }
    web::Json("".to_string())
}

#[get("delete_tag")]
async fn delete_tag(db_pool: Data<DBPool>, params: Query<DeleteRequest>) -> impl Responder {
    for id in params.ids.iter() {
        tag::delete(&db_pool.sqlite_pool, *id).await;
    } // TODO: Err message
    web::Json("")
}

#[get("check_downloaded")]
async fn check_downloaded(db_pool: Data<DBPool>, params: Query<CheckDownloadedQuery>) -> impl Responder {
    if params.blake3.is_empty() {
        return web::Json(CommonResponse {
            err: None,
            ..Default::default()
        });
    }

    if item::get_by_hash(&db_pool.sqlite_pool, params.blake3.as_str())
        .await
        .is_ok()
    {
        web::Json(CommonResponse {
            err: Some("Existed".to_string()),
            ..Default::default()
        })
    } else {
        web::Json(CommonResponse {
            err: None,
            ..Default::default()
        })
    }
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

fn get_relative_path(base_path: &str, path: &Path) -> Result<String, anyhow::Error> {
    let base = PathBuf::from(base_path);
    let path = path.strip_prefix(&base)?;
    Ok(path.to_str().unwrap_or_default().to_string())
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

async fn scan(config: Data<ConfigData>, db_pool: Data<DBPool>) {
    let config = config.config.lock().await;
    let valid_ext = config.extensions.iter().collect::<HashSet<_>>();

    if let Err(e) = item::mark_obsolete_all(&db_pool.sqlite_pool).await {
        error!("Failed to mark all item for reload: {}", e);
        return;
    }

    for (label, base_path) in config.model_paths.iter() {
        let parallelism = Parallelism::RayonNewPool(config.walkdir_parallel);
        for entry in WalkDir::new(base_path)
            .skip_hidden(true)
            .parallelism(parallelism.clone())
            .follow_links(true)
            .into_iter()
            .flatten()
        {
            let path = entry.path();

            let Ok(relative_path) = get_relative_path(base_path, &path) else {
                continue;
            };

            if entry.file_type().is_file() || entry.file_type().is_symlink() {
                let file_ext = path.extension().unwrap_or_default().to_str().unwrap_or_default();
                if valid_ext.contains(&file_ext.to_string()) {
                    save_model_info(&db_pool, &path, label, relative_path.as_str()).await;
                }
            }
        }
    }
}

async fn save_model_info(db_pool: &DBPool, path: &Path, label: &str, relative_path: &str) {
    let mut json_file = PathBuf::from(path);
    json_file.set_extension("json");
    let info = fs::read_to_string(&json_file).await.unwrap_or_default();
    let v: Value = serde_json::from_str(&info).unwrap_or_default();

    let base_model = v["baseModel"].as_str().unwrap_or_default();
    // TODO: Fix compare the real file hash
    let blake3 = v["files"][0]["hashes"]["BLAKE3"].as_str().unwrap_or_default();
    let file_metadata =
        serde_json::from_value::<CivitaiFileMetadata>(v["files"][0]["metadata"].clone()).unwrap_or_default();
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
        blake3,
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
