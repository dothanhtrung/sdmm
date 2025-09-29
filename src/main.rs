//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.
//!
//!
//! TODO:
//! * Replace preview image.
//! * Create folder and move model
//! * Duplicate check

#[cfg(target_os = "linux")]
use tikv_jemallocator::Jemalloc;

#[cfg(target_os = "linux")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

mod api;
mod civitai;
mod config;
mod db;
mod ui;

use crate::civitai::update_model_info;
use crate::config::Config;
use crate::db::DBPool;
use crate::ui::Broadcaster;
use actix_cors::Cors;
use actix_files::Files;
use actix_web::dev::{ServerHandle, ServiceRequest};
use actix_web::error::InternalError;
use actix_web::http::header;
use actix_web::middleware::Condition;
use actix_web::web::Data;
use actix_web::{middleware, web, App, Error, HttpResponse, HttpServer};
use actix_web_httpauth::extractors::basic::BasicAuth;
use actix_web_httpauth::middleware::HttpAuthentication;
use anyhow::anyhow;
use clap::Parser;
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;

const BASE_PATH_PREFIX: &str = "base_";

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Config file path
    #[clap(short, long, default_value = "./sdmm.ron")]
    config: PathBuf,

    /// Update model info
    #[clap(short, long, default_value = "false")]
    update_model_info: bool,
}

struct ConfigData {
    config: RwLock<Config>,
    config_path: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // Subscriber that prints formatted traces to stdout
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_thread_ids(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Parse command line arguments
    let args = Cli::parse();

    // Load config file
    let mut config = load_config(&args.config)?;

    if args.update_model_info {
        update_model_info(&config).await?;
        return Ok(());
    }

    let stop_handle = Arc::new(RwLock::new(StopHandle::default()));

    loop {
        let db_pool = match DBPool::init(&config.db).await {
            Ok(pool) => pool,
            Err(e) => {
                return Err(anyhow!("Failed to connect database: {}.", e,));
            }
        };

        let listen_addr = format!("{}:{}", &config.listen_addr, &config.listen_port);
        let model_paths = config.model_paths.clone();
        let ref_db_pool = Arc::new(db_pool);
        let config_data = Arc::new(ConfigData {
            config: RwLock::new(config.clone()),
            config_path: args.config.clone(),
        });
        let broadcaster = Broadcaster::create();

        let srv = HttpServer::new({
            let stop_handle = stop_handle.clone();
            move || {
                let enable_basic_auth =
                    !config.api.basic_auth_user.is_empty() || !config.api.basic_auth_pass.is_empty();
                let mut app = App::new()
                    .wrap(Cors::default().allow_any_origin())
                    .wrap(Condition::new(
                        enable_basic_auth,
                        HttpAuthentication::basic(basic_auth_validator),
                    ))
                    .wrap(middleware::NormalizePath::trim())
                    .app_data(Data::from(stop_handle.clone()))
                    .app_data(Data::from(ref_db_pool.clone()))
                    .app_data(Data::from(config_data.clone()))
                    .app_data(Data::from(Arc::clone(&broadcaster)));
                for (label, base_path) in model_paths.iter() {
                    app = app.service(
                        Files::new(format!("/{}{}", BASE_PATH_PREFIX, label).as_str(), base_path).show_files_listing(),
                    );
                }

                app = app.service(web::scope("").configure(api::scope_config).configure(ui::scope_config));
                app
            }
        })
        .bind(listen_addr)?
        .run();

        // register the server handle with the stop handle
        stop_handle.read().await.register(srv.handle());

        // run server until stopped (either by ctrl-c or stop endpoint)
        let _ = srv.await;

        if !stop_handle.read().await.is_restarted {
            break;
        }

        stop_handle.write().await.is_restarted = false;

        // Reload config
        config = load_config(&args.config)?;
    }
    Ok(())
}

#[derive(Default)]
struct StopHandle {
    inner: Mutex<Option<ServerHandle>>,
    is_restarted: bool,
}

impl StopHandle {
    /// Sets the server handle to stop.
    pub(crate) fn register(&self, handle: ServerHandle) {
        *self.inner.lock() = Some(handle);
    }

    /// Sends stop signal through contained server handle.
    pub(crate) fn stop(&self, graceful: bool) {
        #[allow(clippy::let_underscore_future)]
        let _ = self.inner.lock().as_ref().unwrap().stop(graceful);
    }
}

fn load_config(config_path: &Path) -> anyhow::Result<Config> {
    if config_path.exists() {
        match Config::load(config_path) {
            Ok(c) => Ok(c),
            Err(e) => Err(anyhow!("Failed to load config file {}: {}", config_path.display(), e)),
        }
    } else {
        let default_config = Config::default();
        default_config.save(config_path, false)?;
        Ok(default_config)
    }
}

async fn basic_auth_validator(
    req: ServiceRequest,
    credentials: BasicAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    let mut user_ok = false;
    let mut pass_ok = false;
    if let Some(config) = req.app_data::<Data<ConfigData>>() {
        let config = config.config.read().await;
        user_ok = config.api.basic_auth_user.is_empty() || credentials.user_id() == config.api.basic_auth_user;
        pass_ok = config.api.basic_auth_pass.is_empty()
            || credentials.password() == Some(config.api.basic_auth_pass.as_str());
    }
    if user_ok && pass_ok {
        Ok(req)
    } else {
        let resp = HttpResponse::Unauthorized()
            .insert_header((header::WWW_AUTHENTICATE, r#"Basic realm="Restricted""#))
            .finish();
        let err = InternalError::from_response("Unauthorized", resp).into();
        Err((err, req))
    }
}
