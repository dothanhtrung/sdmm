//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use crate::api::{CommonResponse, DeleteRequest};
use crate::db;
use crate::db::tag::Tag;
use crate::db::DBPool;
use actix_web::web::Data;
use actix_web::{get, post, web, Responder};
use actix_web_lab::extract::Query;
use serde::Serialize;
use std::collections::HashSet;
use tracing::error;

pub fn scope(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/tag")
            .service(get_all)
            .service(get)
            .service(update)
            .service(delete),
    );
}

#[derive(Serialize, Default)]
struct TagResponse {
    tag: Option<Tag>,
    err: Option<String>,
}

#[get("")]
async fn get_all(db_pool: Data<DBPool>) -> impl Responder {
    let get_all = db::tag::list_tags(&db_pool.sqlite_pool, HashSet::new())
        .await
        .unwrap_or_else(|e| {
            error!("Failed to list tags: {e}");
            Vec::new()
        });
    web::Json(get_all)
}

#[get("detail/{tag}")]
async fn get(db_pool: Data<DBPool>, tag: web::Path<String>) -> impl Responder {
    let res = match db::tag::get_tag_by_name(&db_pool.sqlite_pool, tag.into_inner().as_str()).await {
        Ok(tag) => TagResponse {
            tag: Some(tag),
            ..Default::default()
        },
        Err(e) => TagResponse {
            err: Some(format!("{e}")),
            ..Default::default()
        },
    };
    web::Json(res)
}

#[post("update")]
async fn update(db_pool: Data<DBPool>, data: web::Json<Tag>) -> impl Responder {
    if let Err(e) = db::tag::update_tag(&db_pool.sqlite_pool, &data.into_inner()).await {
        return web::Json(CommonResponse {
            err: Some(format!("Failed to update tag: {e}")),
            ..Default::default()
        });
    }
    web::Json(CommonResponse::default())
}

#[get("delete")]
async fn delete(db_pool: Data<DBPool>, params: Query<DeleteRequest>) -> impl Responder {
    let mut err_str = String::new();
    for id in params.ids.iter() {
        if let Err(e) = db::tag::delete(&db_pool.sqlite_pool, *id).await {
            err_str.push_str(&format!("{e}\n"));
        }
    }

    let err = if err_str.is_empty() { None } else { Some(err_str) };
    web::Json(CommonResponse {
        err,
        ..Default::default()
    })
}
