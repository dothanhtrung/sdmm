use crate::api::CommonResponse;
use crate::config::Config;
use crate::ConfigData;
use actix_web::web::Data;
use actix_web::{get, post, web, Responder};

pub fn scope(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/config").service(get).service(update));
}

#[get("")]
async fn get(config_data: Data<ConfigData>) -> impl Responder {
    let config = (*config_data.config.read().await).clone();
    web::Json(config)
}

#[post("update")]
async fn update(config_data: Data<ConfigData>, data: web::Json<Config>) -> impl Responder {
    let mut config = config_data.config.write().await;
    *config = data.into_inner();
    if let Err(e) = config.save(&config_data.config_path, true) {
        web::Json(CommonResponse {
            err: Some(format!("Failed to save config: {e}")),
            ..Default::default()
        })
    } else {
        web::Json(CommonResponse {
            msg: "Config updated".to_string(),
            ..Default::default()
        })
    }
}
