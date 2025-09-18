//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use ron::ser::{to_string_pretty, PrettyConfig};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tracing::log::info;

const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0";
const DEFAULT_LISTEN_PORT: u32 = 9696;

const DEFAULT_SQLITE_PATH: &str = "sdmm.sqlite";

const DEFAULT_API_PER_PAGE: u32 = 20;
const DEFAULT_PARALLEL: usize = 8;

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
    pub basic_auth_user: String,
    pub basic_auth_pass: String,
}

impl Default for APIConfig {
    fn default() -> Self {
        Self {
            per_page: DEFAULT_API_PER_PAGE,
            basic_auth_user: String::new(),
            basic_auth_pass: String::new(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CivitaiSearch {
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub base_models: Vec<String>,
    #[serde(default)]
    pub sort: String, // e.g. "Newest", "Most Downloaded"
    #[serde(default)]
    pub nsfw: bool,
    #[serde(default)]
    pub per_page: usize,
}

impl Default for CivitaiSearch {
    fn default() -> Self {
        Self {
            types: Vec::new(),
            base_models: Vec::new(),
            sort: String::new(),
            nsfw: false,
            per_page: 20,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CivitaiConfig {
    pub api_key: String,
    pub overwrite_thumbnail: bool,
    pub overwrite_json: bool,
    #[serde(default)]
    pub download_dir: HashMap<String, String>,
    #[serde(default)]
    pub max_retries: usize,
    #[serde(default)]
    pub search: CivitaiSearch,
}

impl Default for CivitaiConfig {
    fn default() -> Self {
        Self {
            api_key: "your_civitai_api_key".to_string(),
            overwrite_thumbnail: false,
            overwrite_json: false,
            download_dir: HashMap::new(),
            max_retries: 3,
            search: CivitaiSearch::default(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub db: DBConfig,
    pub model_paths: HashMap<String, String>,
    #[serde(default)]
    pub civitai: CivitaiConfig,
    #[serde(default)]
    pub listen_addr: String,
    #[serde(default)]
    pub listen_port: u32,
    #[serde(default)]
    pub api: APIConfig,
    #[serde(default)]
    pub parallel: usize,
    #[serde(default)]
    pub extensions: HashSet<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: DEFAULT_LISTEN_ADDR.to_string(),
            listen_port: DEFAULT_LISTEN_PORT,
            parallel: DEFAULT_PARALLEL,
            model_paths: HashMap::from([("collection1".to_string(), "/workspace/models".to_string())]),
            extensions: HashSet::from([
                "safetensors".to_string(),
                "ckpt".to_string(),
                "gguf".to_string(),
                "pt".to_string(),
                "pth".to_string(),
            ]),
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
