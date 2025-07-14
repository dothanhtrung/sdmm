//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use crate::api::SearchQuery;
use crate::ConfigData;
use actix_files::Files;
use actix_web::web::Data;
use actix_web::{get, web, HttpResponse, Responder};
use actix_web_lab::extract::Query;
use tera::Tera;

pub fn scope_config(cfg: &mut web::ServiceConfig) {
    let tera = Tera::new("res/html/**/*").unwrap();
    cfg.app_data(Data::new(tera))
        .service(index)
        .service(get_item)
        .service(maintenance)
        .service(civitai)
        .service(tag)
        .service(setting)
        .service(Files::new("/assets", "res/assets"))
        .service(Files::new("/css", "res/css"))
        .service(Files::new("/js", "res/js"));
}

#[get("/")]
async fn index(tmpl: Data<Tera>, query_params: Query<SearchQuery>) -> impl Responder {
    let mut ctx = tera::Context::new();
    ctx.insert("search", &query_params.search.clone().unwrap_or_default());

    match tmpl.render("index.html", &ctx) {
        Ok(template) => HttpResponse::Ok().content_type("text/html").body(template),
        Err(e) => HttpResponse::Ok()
            .content_type("text/html")
            .body(format!("Template error: {e}")),
    }
}

#[get("/item/{id}")]
async fn get_item(tmpl: Data<Tera>, id: web::Path<i64>) -> impl Responder {
    let mut ctx = tera::Context::new();
    ctx.insert("id", &id.into_inner());
    match tmpl.render("item.html", &ctx) {
        Ok(template) => HttpResponse::Ok().content_type("text/html").body(template),
        Err(e) => HttpResponse::Ok()
            .content_type("text/html")
            .body(format!("Template error: {e}")),
    }
}

#[get("/maintenance")]
async fn maintenance(tmpl: Data<Tera>) -> impl Responder {
    let ctx = tera::Context::new();
    match tmpl.render("maintenance.html", &ctx) {
        Ok(template) => HttpResponse::Ok().content_type("text/html").body(template),
        Err(e) => HttpResponse::Ok()
            .content_type("text/html")
            .body(format!("Template error: {e}")),
    }
}

#[get("/civitai")]
async fn civitai(tmpl: Data<Tera>, config_data: Data<ConfigData>) -> impl Responder {
    let mut ctx = tera::Context::new();
    let config = config_data.config.read().await;
    ctx.insert("config", &config.civitai);
    match tmpl.render("civitai.html", &ctx) {
        Ok(template) => HttpResponse::Ok().content_type("text/html").body(template),
        Err(e) => HttpResponse::Ok()
            .content_type("text/html")
            .body(format!("Template error: {e}")),
    }
}

#[get("/tag/{name}")]
async fn tag(tmpl: Data<Tera>) -> impl Responder {
    let ctx = tera::Context::new();
    match tmpl.render("tag.html", &ctx) {
        Ok(template) => HttpResponse::Ok().content_type("text/html").body(template),
        Err(e) => HttpResponse::Ok()
            .content_type("text/html")
            .body(format!("Template error: {e}")),
    }
}

#[get("/setting")]
async fn setting(tmpl: Data<Tera>) -> impl Responder {
    let ctx = tera::Context::new();
    match tmpl.render("config.html", &ctx) {
        Ok(template) => HttpResponse::Ok().content_type("text/html").body(template),
        Err(e) => HttpResponse::Ok()
            .content_type("text/html")
            .body(format!("Template error: {e}")),
    }
}
