//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use crate::api::TRASH_DIR;
use crate::civitai::update_model_info;
use crate::db::job::{add_job, update_job, JobState};
use crate::db::DBPool;
use crate::ui::Broadcaster;
use crate::{api, db, ConfigData, StopHandle};
use actix_web::web::Data;
use actix_web::{get, rt, web, HttpResponse, Responder};
use jwalk::{Parallelism, WalkDir};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{RwLock, Semaphore};
use tracing::{error, info};

pub fn scope(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/maintenance")
            .service(scan_folder)
            .service(remove_orphan)
            .service(sync_civitai)
            .service(restart)
            .service(force_restart)
            .service(empty_trash),
    );
}

#[get("scan")]
async fn scan_folder(
    config: Data<ConfigData>,
    db_pool: Data<DBPool>,
    broadcaster: Data<Broadcaster>,
) -> impl Responder {
    rt::spawn(async move {
        broadcaster.warn("Start scanning folder...").await;
        scan(config, db_pool, &broadcaster).await;
    });
    web::Json("")
}

#[get("remove_orphan")]
async fn remove_orphan(db_pool: Data<DBPool>, broadcaster: Data<Broadcaster>) -> impl Responder {
    broadcaster.warn("Removing orphan...").await;
    let deleted_items = db::item::clean(&db_pool.sqlite_pool).await.unwrap_or_default();

    web::Json(format!(
        "{{
        \"deleted_items\": {},
    }}",
        deleted_items,
    ))
}

#[get("sync_civitai")]
async fn sync_civitai(
    config_data: Data<ConfigData>,
    db_pool: Data<DBPool>,
    broadcaster: Data<Broadcaster>,
) -> impl Responder {
    rt::spawn(async move {
        broadcaster.warn("Start to sync Civitai...").await;
        let id = add_job(&db_pool.sqlite_pool, "Sync Civitai", "").await;
        let config = config_data.config.read().await.clone();
        let _ = update_model_info(&config).await;
        if let Ok(id) = id {
            let _ = update_job(&db_pool.sqlite_pool, id, "", JobState::Succeed).await;
        }
        broadcaster.info("Sync Civitai finished").await;
        scan(config_data, db_pool, &broadcaster).await;
    });
    web::Json("")
}

#[get("empty_trash")]
async fn empty_trash(config: Data<ConfigData>, broadcaster: Data<Broadcaster>) -> impl Responder {
    broadcaster.warn("Emptying trash...").await;
    let config = config.config.read().await;
    for (_, base_path) in config.model_paths.iter() {
        let trash_dir = PathBuf::from(base_path).join(TRASH_DIR);
        if let Err(e) = fs::remove_dir_all(&trash_dir).await {
            error!("Failed to remove trash directory: {}", e);
        }
    }
    web::Json("")
}

#[get("restart")]
async fn restart(stop_handle: Data<RwLock<StopHandle>>, broadcaster: Data<Broadcaster>) -> impl Responder {
    broadcaster.warn("Restarting server. Please wait a minute...").await;
    let mut stop_handle = stop_handle.write().await;
    stop_handle.is_restarted = true;
    stop_handle.stop(true);
    HttpResponse::NoContent().finish()
}

#[get("force_restart")]
async fn force_restart(stop_handle: Data<RwLock<StopHandle>>, broadcaster: Data<Broadcaster>) -> impl Responder {
    broadcaster.warn("Force restarting server...").await;
    let mut stop_handle = stop_handle.write().await;
    stop_handle.is_restarted = true;
    stop_handle.stop(false);
    HttpResponse::NoContent().finish()
}

async fn scan(config: Data<ConfigData>, db_pool: Data<DBPool>, broadcaster: &Broadcaster) {
    let id = add_job(&db_pool.sqlite_pool, "Scan folder", "").await;

    let config = config.config.read().await;
    let valid_ext = config.extensions.iter().collect::<HashSet<_>>();

    if let Err(e) = db::item::mark_obsolete_all(&db_pool.sqlite_pool).await {
        let msg = format!("Failed to mark all item for reload: {e}");
        if let Ok(id) = id {
            let _ = update_job(&db_pool.sqlite_pool, id, msg.as_str(), JobState::Failed).await;
        }
        broadcaster.error(format!("Scan failed. {}", &msg).as_str()).await;
        error!(msg);
        return;
    }
    let mut handles = Vec::new();
    let semaphore = Arc::new(Semaphore::new(config.parallel));
    for (label, base_path) in config.model_paths.iter() {
        let parallelism = Parallelism::RayonNewPool(config.parallel);
        for entry in WalkDir::new(base_path)
            .skip_hidden(true)
            .parallelism(parallelism.clone())
            .follow_links(false)
            .into_iter()
            .flatten()
        {
            if entry.file_type().is_file() {
                let path = entry.path();
                let Ok(relative_path) = api::get_relative_path(base_path, &path) else {
                    continue;
                };

                let file_ext = path.extension().unwrap_or_default().to_str().unwrap_or_default();
                if valid_ext.contains(&file_ext.to_string()) {
                    let semaphore = semaphore.clone();
                    let db_pool = db_pool.clone();
                    let label = label.clone();

                    let handle = tokio::spawn(async move {
                        if let Ok(_permit) = semaphore.acquire().await {
                            info!("Found {path:?}");
                            api::save_model_info(&db_pool, &path, label.as_str(), relative_path.as_str()).await;
                        }
                    });
                    handles.push(handle);
                }
            }
        }
        info!("Finished scanning {}", label);
    }

    for handle in handles {
        if let Err(e) = handle.await {
            error!("Failed to scan model: {e}");
        }
    }

    if let Ok(id) = id {
        let _ = update_job(&db_pool.sqlite_pool, id, "", JobState::Succeed).await;
    }
    broadcaster.info("Scan finished").await;
}
