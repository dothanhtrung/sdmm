use crate::ConfigData;
use crate::api::CommonResponse;
use crate::civitai::CivitaiEnums;
use crate::config::Config;
use actix_web::web::Data;
use actix_web::{Responder, get, post, web};
use reqwest::Client;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use tracing::error;

pub fn scope(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/config").service(get).service(update));
}

#[get("")]
async fn get(config_data: Data<ConfigData>) -> impl Responder {
    let mut config = (*config_data.config.read().await).clone();

    // Get baseModels and types list
    if config.civitai.base_models.is_empty() || config.civitai.types.is_empty() {
        let client = Client::new();
        let mut headers = HeaderMap::new();
        if let Ok(bearer) = HeaderValue::from_str(&format!("Bearer {}", config.civitai.api_key)) {
            headers.insert(AUTHORIZATION, bearer);
        }
        let url = "https://civitai.com/api/v1/enums";
        match client.get(url).headers(headers).send().await {
            Ok(response) => match response.json::<CivitaiEnums>().await {
                Ok(info) => {
                    config.civitai.base_models = info.active_base_model.clone();
                    config.civitai.types = info.model_type.clone();
                }
                Err(e) => error!("Failed to parse civitai enums: {}", e),
            },
            Err(e) => error!("Failed to fetch {}: {}", url, e),
        }
    }

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
