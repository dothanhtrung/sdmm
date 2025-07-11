use crate::civitai::update_model_info;
use crate::db::DBPool;
use crate::{api, db, ConfigData};
use actix_web::web::Data;
use actix_web::{get, rt, web, Responder};
use std::collections::HashSet;
use std::path::PathBuf;
use tracing::error;
use jwalk::{Parallelism, WalkDir};
use tokio::fs;
use crate::api::TRASH_DIR;

pub fn scope(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/maintenance")
            .service(scan_folder)
            .service(remove_orphan)
            .service(sync_civitai)
            .service(empty_trash)
        ,
    );
}

#[get("scan")]
async fn scan_folder(config: Data<ConfigData>, db_pool: Data<DBPool>) -> impl Responder {
    rt::spawn(async move {
        scan(config, db_pool).await;
    });
    web::Json("")
}

#[get("remove_orphan")]
async fn remove_orphan(db_pool: Data<DBPool>) -> impl Responder {
    let deleted_items = db::item::clean(&db_pool.sqlite_pool).await.unwrap_or_default();

    web::Json(format!(
        "{{
        \"deleted_items\": {},
    }}",
        deleted_items,
    ))
}

#[get("sync_civitai")]
async fn sync_civitai(config_data: Data<ConfigData>, db_pool: Data<DBPool>) -> impl Responder {
    rt::spawn(async move {
        let config = config_data.config.read().await.clone();
        let _ = update_model_info(&config).await;
        scan(config_data, db_pool).await;
    });
    web::Json("")
}

#[get("empty_trash")]
async fn empty_trash(config: Data<ConfigData>) -> impl Responder {
    let config = config.config.read().await;
    for (_, base_path) in config.model_paths.iter() {
        let trash_dir = PathBuf::from(base_path).join(TRASH_DIR);
        if let Err(e) = fs::remove_dir_all(&trash_dir).await {
            error!("Failed to remove trash directory: {}", e);
        }
    }
    web::Json("")
}

async fn scan(config: Data<ConfigData>, db_pool: Data<DBPool>) {
    let config = config.config.read().await;
    let valid_ext = config.extensions.iter().collect::<HashSet<_>>();

    if let Err(e) = db::item::mark_obsolete_all(&db_pool.sqlite_pool).await {
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

            let Ok(relative_path) = api::get_relative_path(base_path, &path) else {
                continue;
            };

            if entry.file_type().is_file() || entry.file_type().is_symlink() {
                let file_ext = path.extension().unwrap_or_default().to_str().unwrap_or_default();
                if valid_ext.contains(&file_ext.to_string()) {
                    api::save_model_info(&db_pool, &path, label, relative_path.as_str()).await;
                }
            }
        }
    }
}