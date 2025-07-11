//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use crate::api::SearchQuery;
use crate::ConfigData;
use actix_files::Files;
use actix_web::web::Data;
use actix_web::{error, get, web, HttpResponse, Responder};
use actix_web_lab::extract::Query;
use tera::Tera;

pub fn scope_config(cfg: &mut web::ServiceConfig) {
    let tera = Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/res/html/**/*")).unwrap();
    cfg.app_data(Data::new(tera))
        .service(index)
        .service(get_item)
        .service(maintenance)
        .service(civitai)
        .service(tag)
        .service(setting)
        .service(Files::new(
            "/assets",
            concat!(env!("CARGO_MANIFEST_DIR"), "/res/assets"),
        ))
        .service(Files::new("/css", concat!(env!("CARGO_MANIFEST_DIR"), "/res/css")))
        .service(Files::new("/js", concat!(env!("CARGO_MANIFEST_DIR"), "/res/js")));
}

#[get("/")]
async fn index(tmpl: Data<Tera>, query_params: Query<SearchQuery>) -> impl Responder {
    let mut ctx = tera::Context::new();
    ctx.insert("search", &query_params.search.clone().unwrap_or_default());

    let template = tmpl
        .render("index.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(format!("Template error: {:?}", e)))
        .unwrap_or_default();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[get("/item/{id}")]
async fn get_item(tmpl: Data<Tera>, id: web::Path<i64>) -> impl Responder {
    let mut ctx = tera::Context::new();
    ctx.insert("id", &id.into_inner());
    let template = tmpl
        .render("item.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(format!("Template error: {:?}", e)))
        .unwrap_or_default();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[get("/maintenance")]
async fn maintenance(tmpl: Data<Tera>) -> impl Responder {
    let ctx = tera::Context::new();
    let template = tmpl
        .render("maintenance.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(format!("Template error: {:?}", e)))
        .unwrap_or_default();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[get("/civitai")]
async fn civitai(tmpl: Data<Tera>, config_data: Data<ConfigData>) -> impl Responder {
    let mut ctx = tera::Context::new();
    let config = config_data.config.read().await;
    ctx.insert("token", &config.civitai.api_key);
    let template = tmpl
        .render("civitai.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(format!("Template error: {:?}", e)))
        .unwrap_or_default();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[get("/tag/{name}")]
async fn tag(tmpl: Data<Tera>) -> impl Responder {
    let ctx = tera::Context::new();
    // TODO: Print template error
    let template = tmpl
        .render("tag.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(format!("Template error: {e:?}")))
        .unwrap_or_default();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[get("/setting")]
async fn setting(tmpl: Data<Tera>) -> impl Responder {
    let ctx = tera::Context::new();
    let template = tmpl.render("config.html", &ctx).unwrap_or_default();
    HttpResponse::Ok().content_type("text/html").body(template)
}
