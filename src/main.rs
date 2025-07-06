//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.
//!
//!
//! TODO:
//! * Replace preview image. Add note.
//! * Create folder and move model
//! * Tag
//!   * Tag depend
//! * Browsing and download model from Civitai
//!   * Check if file is downloaded
//! * Download new version of model from Civitai
//! * Duplicate check

#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

mod api;
mod civitai;
mod config;
mod db;
mod ui;

use crate::api::scan_folder;
use crate::civitai::update_model_info;
use crate::config::Config;
use crate::db::DBPool;
use actix_cors::Cors;
use actix_files::Files;
use actix_web::web::Data;
use actix_web::{middleware, web, App, HttpServer};
use anyhow::anyhow;
use clap::Parser;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{error, warn};
use tracing_subscriber::EnvFilter;

const BASE_PATH_PREFIX: &str = "base_";

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Config file path
    #[clap(short, long, default_value = "./sdmm.ron")]
    config: PathBuf,

    /// Export default config to file
    #[clap(short, long)]
    export_config: Option<PathBuf>,

    /// Update model info
    #[clap(short, long, default_value = "false")]
    update_model_info: bool,
}

struct ConfigData {
    config: Mutex<Config>,
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

    // Export default config to file.
    // Useful when the config file is lost or the config schema is changed between major version.
    if let Some(export_config_path) = &args.export_config {
        Config::default().save(export_config_path, false)?;
        return Ok(());
    }

    // Load config file
    let config_path = Path::new(&args.config);
    let config = if config_path.exists() {
        match Config::load(&args.config) {
            Ok(c) => c,
            Err(e) => return Err(anyhow!("Failed to load config file {}: {}", &args.config.display(), e)),
        }
    } else {
        let default_config = Config::default();
        default_config.save(config_path, false)?;
        default_config
    };

    if args.update_model_info {
        update_model_info(config.clone()).await?;
        return Ok(());
    }

    let db_pool;
    loop {
        match DBPool::init(&config.db).await {
            Ok(pool) => {
                db_pool = pool;
                break;
            }
            Err(e) => {
                error!(
                    "Failed to init DB Pool: {}. Retry in {} seconds",
                    e, config.db.init_timeout_secs
                );
                sleep(Duration::from_secs(config.db.init_timeout_secs)).await;
            }
        }
    }

    let listen_addr = format!("{}:{}", &config.listen_addr, &config.listen_port);
    let model_paths = config.model_paths.clone();
    let ref_db_pool = Arc::new(db_pool);
    let config_data = Arc::new(ConfigData {
        config: Mutex::new(config.clone()),
        config_path: args.config.clone(),
    });

    HttpServer::new(move || {
        let mut app = App::new()
            .wrap(Cors::default().allow_any_origin())
            .app_data(Data::from(ref_db_pool.clone()))
            .app_data(Data::from(config_data.clone()))
            .wrap(middleware::NormalizePath::trim());
        for (label, base_path) in model_paths.iter() {
            app = app.service(
                Files::new(format!("/{}{}", BASE_PATH_PREFIX, label).as_str(), base_path).show_files_listing(),
            );
        }

        app = app.service(web::scope("").configure(api::scope_config).configure(ui::scope_config));
        app
    })
    .bind(listen_addr)?
    .run()
    .await?;

    Ok(())
}
