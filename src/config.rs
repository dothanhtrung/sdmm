//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use ron::ser::{PrettyConfig, to_string_pretty};
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
    #[serde(default)]
    pub basic_auth_user: String,
    #[serde(default)]
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
    #[serde(default = "default_base_models")]
    pub base_models: Vec<String>,
    #[serde(default = "default_model_types")]
    pub types: Vec<String>,
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
            base_models: Vec::new(),
            types: Vec::new(),
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

fn default_base_models() -> Vec<String> {
    vec![
        String::from("Anima"),
        String::from("AuraFlow"),
        String::from("Chroma"),
        String::from("CogVideoX"),
        String::from("Flux.1 S"),
        String::from("Flux.1 D"),
        String::from("Flux.1 Krea"),
        String::from("Flux.1 Kontext"),
        String::from("Flux.2 D"),
        String::from("Flux.2 Klein 9B"),
        String::from("Flux.2 Klein 9B-base"),
        String::from("Flux.2 Klein 4B"),
        String::from("Flux.2 Klein 4B-base"),
        String::from("HiDream"),
        String::from("Hunyuan 1"),
        String::from("Hunyuan Video"),
        String::from("Illustrious"),
        String::from("Imagen4"),
        String::from("Kling"),
        String::from("Kolors"),
        String::from("LTXV"),
        String::from("LTXV2"),
        String::from("Lumina"),
        String::from("Mochi"),
        String::from("Nano Banana"),
        String::from("NoobAI"),
        String::from("ODOR"),
        String::from("OpenAI"),
        String::from("Other"),
        String::from("PixArt a"),
        String::from("PixArt E"),
        String::from("Playground v2"),
        String::from("Pony"),
        String::from("Pony V7"),
        String::from("Qwen"),
        String::from("Stable Cascade"),
        String::from("SD 1.4"),
        String::from("SD 1.5"),
        String::from("SD 1.5 LCM"),
        String::from("SD 1.5 Hyper"),
        String::from("SD 2.0"),
        String::from("SD 2.0 768"),
        String::from("SD 2.1"),
        String::from("SD 2.1 768"),
        String::from("SD 2.1 Unclip"),
        String::from("SD 3"),
        String::from("SD 3.5"),
        String::from("SD 3.5 Large"),
        String::from("SD 3.5 Large Turbo"),
        String::from("SD 3.5 Medium"),
        String::from("Sora 2"),
        String::from("SDXL 0.9"),
        String::from("SDXL 1.0"),
        String::from("SDXL 1.0 LCM"),
        String::from("SDXL Lightning"),
        String::from("SDXL Hyper"),
        String::from("SDXL Turbo"),
        String::from("SDXL Distilled"),
        String::from("Seedance"),
        String::from("Seedream"),
        String::from("SVD"),
        String::from("SVD XT"),
        String::from("Veo 3"),
        String::from("Vidu Q1"),
        String::from("Wan Video"),
        String::from("Wan Video 1.3B t2v"),
        String::from("Wan Video 14B t2v"),
        String::from("Wan Video 14B i2v 480p"),
        String::from("Wan Video 14B i2v 720p"),
        String::from("Wan Video 2.2 TI2V-5B"),
        String::from("Wan Video 2.2 I2V-A14B"),
        String::from("Wan Video 2.2 T2V-A14B"),
        String::from("Wan Video 2.5 T2V"),
        String::from("Wan Video 2.5 I2V"),
        String::from("ZImageTurbo"),
        String::from("ZImageBase"),
    ]
}

fn default_model_types() -> Vec<String> {
    vec![
        String::from("Checkpoint"),
        String::from("TextualInversion"),
        String::from("Hypernetwork"),
        String::from("AestheticGradient"),
        String::from("LORA"),
        String::from("LoCon"),
        String::from("DoRA"),
        String::from("Controlnet"),
        String::from("Upscaler"),
        String::from("MotionModule"),
        String::from("VAE"),
        String::from("Poses"),
        String::from("Wildcards"),
        String::from("Workflows"),
        String::from("Detection"),
        String::from("Other"),
    ]
}
