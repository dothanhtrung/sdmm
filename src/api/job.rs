use crate::api::CommonResponse;
use crate::db::job::Job;
use crate::db::DBPool;
use crate::{db, ConfigData};
use actix_web::web::{Data, Query};
use actix_web::{get, web, Responder};
use serde::{Deserialize, Serialize};

pub fn scope(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/job").service(get_jobs).service(clear_jobs));
}

#[derive(Deserialize)]
struct JobQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Serialize, Default)]
struct JobResponse {
    jobs: Vec<Job>,
    total: i64,
    err: Option<String>,
}

#[get("")]
async fn get_jobs(
    config_data: Data<ConfigData>,
    db_pool: Data<DBPool>,
    query_params: Query<JobQuery>,
) -> impl Responder {
    let config = config_data.config.read().await;
    let limit = query_params.limit.unwrap_or(config.api.per_page as i64);
    let offset = query_params.offset.unwrap_or(0);
    let mut res = JobResponse::default();
    match db::job::get(&db_pool.sqlite_pool, limit, offset).await {
        Ok((jobs, total)) => {
            res.jobs = jobs;
            res.total = total;
        }
        Err(e) => {
            res.err = Some(format!("Failed to get jobs list: {}", e));
        }
    }
    web::Json(res)
}

#[get("clear")]
async fn clear_jobs(db_pool: Data<DBPool>) -> impl Responder {
    let msg = if let Err(e) = db::job::clean(&db_pool.sqlite_pool).await {
        CommonResponse::from_err(format!("{e}").as_str());
    } else {
        CommonResponse::from_msg("Clear jobs successfully");
    };
    web::Json(msg)
}
