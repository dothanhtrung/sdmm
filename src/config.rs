//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use ron::ser::{to_string_pretty, PrettyConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tracing::log::info;

const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0";
const DEFAULT_LISTEN_PORT: u32 = 9696;

const DEFAULT_SQLITE_PATH: &str = "sdmm.sqlite";

const DEFAULT_API_PER_PAGE: u32 = 20;

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct SQLiteConfig {
    pub db_path: String,
}

impl Default for SQLiteConfig {
    fn default() -> Self {
        Self {
            db_path: DEFAULT_SQLITE_PATH.to_string(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct DBConfig {
    pub sqlite: SQLiteConfig,
}

#[derive(Clone, Deserialize, Debug, Serialize)]
pub struct APIConfig {
    pub per_page: u32,
}

impl Default for APIConfig {
    fn default() -> Self {
        Self {
            per_page: DEFAULT_API_PER_PAGE,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CivitaiConfig {
    pub api_key: String,
    pub overwrite_thumbnail: bool,
    pub overwrite_json: bool,
    #[serde(default)]
    pub saved_location: HashMap<String, String>,
}

impl Default for CivitaiConfig {
    fn default() -> Self {
        Self {
            api_key: "your_civitai_api_key".to_string(),
            overwrite_thumbnail: false,
            overwrite_json: false,
            saved_location: HashMap::new(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub db: DBConfig,
    pub model_paths: HashMap<String, String>,
    pub civitai: CivitaiConfig,
    #[serde(default)]
    pub listen_addr: String,
    #[serde(default)]
    pub listen_port: u32,
    pub api: APIConfig,
    #[serde(default)]
    pub walkdir_parallel: usize,
    #[serde(default)]
    pub extensions: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: DEFAULT_LISTEN_ADDR.to_string(),
            listen_port: DEFAULT_LISTEN_PORT,
            walkdir_parallel: 8,
            model_paths: HashMap::from([("collection1".to_string(), "/workspace/models".to_string())]),
            extensions: vec![
                "safetensors".to_string(),
                "ckpt".to_string(),
                "gguf".to_string(),
                "pt".to_string(),
                "pth".to_string(),
            ],
            db: DBConfig::default(),
            api: APIConfig::default(),
            civitai: CivitaiConfig::default(),
        }
    }
}

impl Config {
    /// Load config from file
    pub fn load(config_path: &Path) -> anyhow::Result<Self> {
        let file = File::open(config_path)?;
        let config = ron::de::from_reader(file)?;
        Ok(config)
    }

    /// Save config to file
    pub fn save(&self, config_path: &Path, force_overwrite: bool) -> anyhow::Result<()> {
        info!("Saving config file to: {:?}", config_path.display());
        if config_path.exists() && !force_overwrite {
            return Err(anyhow::anyhow!("Do not save. Configuration file already exists"));
        }

        let pretty = PrettyConfig::default();
        let ron_str = to_string_pretty(self, pretty)?;

        if let Some(parent_dir) = config_path.parent() {
            std::fs::create_dir_all(parent_dir)?;
        }
        let mut file = File::create(config_path)?;
        file.write_all(ron_str.as_bytes()).map_err(|e| anyhow::anyhow!(e))
    }
}
