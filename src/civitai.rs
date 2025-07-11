//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use crate::api::TRASH_DIR;
use crate::config::Config;
use actix_web_lab::__reexports::futures_util::StreamExt;
use jwalk::{Parallelism, WalkDir};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{to_string_pretty, Value};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs;
use tokio::sync::Semaphore;
use tracing::{error, info};

pub const PREVIEW_EXT: &str = "jpeg";

#[derive(PartialEq)]
pub enum FileType {
    NA,
    Video,
    Image,
}

#[derive(Deserialize, Default)]
pub struct CivitaiFileMetadata {
    pub format: String,
    pub fp: Option<String>,
    pub size: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct CivitaiModel {
    pub name: String,
    pub nsfw: bool,
    pub poi: bool,
    #[serde(rename = "type")]
    pub model_type: String,
}

pub async fn update_model_info(config: &Config) -> anyhow::Result<()> {
    let valid_ext = config.extensions.iter().collect::<HashSet<_>>();
    let client = Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", config.civitai.api_key))?,
    );

    let semaphore = Arc::new(Semaphore::new(config.walkdir_parallel));
    let parallelism = Parallelism::RayonNewPool(config.walkdir_parallel);
    for (_, base_path) in config.model_paths.iter() {
        for entry in WalkDir::new(base_path)
            .skip_hidden(true)
            .parallelism(parallelism.clone())
            .follow_links(true)
            .into_iter()
            .flatten()
        {
            let path = entry.path();
            if entry.file_type().is_file() || entry.file_type().is_symlink() {
                let file_ext = path.extension().unwrap_or_default().to_str().unwrap_or_default();
                if valid_ext.contains(&file_ext.to_string()) {
                    let client = client.clone();
                    let headers = headers.clone();
                    let config = config.clone();
                    let semaphore = semaphore.clone();

                    tokio::spawn(async move {
                        info!("Update model info: {}", entry.path().display());
                        if let Ok(_permit) = semaphore.acquire().await {
                            if let Err(e) = get_model_info(&path, &client, &headers, None, &config).await {
                                error!("Failed to get model info: {}", e);
                            }
                        }
                    });
                }
            }
        }
    }

    Ok(())
}

pub async fn get_model_info(
    path: &Path,
    client: &Client,
    headers: &HeaderMap,
    blake3: Option<String>,
    config: &Config,
) -> anyhow::Result<()> {
    let info: Value;
    let mut json_path = PathBuf::from(path);
    json_path.set_extension("json");

    if !json_path.exists() || config.civitai.overwrite_json {
        let hash = blake3.unwrap_or(calculate_blake3(path)?);
        let url = format!("https://civitai.com/api/v1/model-versions/by-hash/{hash}");
        info = client.get(url).headers(headers.clone()).send().await?.json().await?;
        if let Some(err) = info["error"].as_str() {
            if !err.is_empty() {
                return Err(anyhow::anyhow!(err.to_string()));
            }
        }
        save_info(&json_path, &info).await?;
    } else {
        info!("File already exists: {}", json_path.display());
        info = serde_json::from_reader(File::open(&json_path)?)?;
    }

    download_preview(client, headers, config, &info, path).await?;

    Ok(())
}

pub async fn download_file(
    url: &str,
    path: &Path,
    client: &Client,
    headers: &HeaderMap,
    base_paths: &HashMap<String, String>,
) -> anyhow::Result<()> {
    if path.exists() {
        let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_millis();
        let mut trash_path = PathBuf::from(path.parent().unwrap_or(Path::new("."))).join(TRASH_DIR);
        for (_, base_path) in base_paths.iter() {
            if path.starts_with(base_path) {
                trash_path = PathBuf::from(base_path).join(TRASH_DIR);
            }
        }
        let mut new_name = PathBuf::from(path);
        new_name.set_extension(format!(
            "{}.bakup.{}",
            path.extension().unwrap_or_default().to_str().unwrap_or_default(),
            timestamp
        ));
        trash_path = trash_path.join(new_name.file_name().unwrap());
        fs::rename(path, trash_path).await?;
    }

    let mut file = File::create(path)?;
    let response = client.get(url).headers(headers.clone()).send().await?;
    let mut stream = response.bytes_stream();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        file.write_all(&chunk)?;
    }
    file.flush()?;
    info!("File downloaded: {}", path.display());
    Ok(())
}

async fn download_preview(
    client: &Client,
    headers: &HeaderMap,
    config: &Config,
    info: &Value,
    model_path: &Path,
) -> anyhow::Result<()> {
    if let Some(images) = info["images"].as_array() {
        if let Some(first_image) = images.first() {
            if let Some(url) = first_image["url"].as_str() {
                let extension = get_extension_from_url(url).unwrap_or(PREVIEW_EXT.to_string());
                let mut preview_file = PathBuf::from(model_path);
                preview_file.set_extension(extension);

                let image_path = Path::new(&preview_file);
                if image_path.exists() && !config.civitai.overwrite_thumbnail {
                    info!("File already exists: {}", image_path.display());
                } else {
                    download_file(url, image_path, client, headers, &config.model_paths).await?;
                }

                let file_type = file_type(image_path).await;
                if file_type == FileType::Video {
                    generate_video_thumbnail(&preview_file, config.civitai.overwrite_thumbnail)?;
                } else if file_type == FileType::Image {
                    //  Change preview image extension to jpeg for easier to manage
                    if image_path.extension().unwrap_or_default() != PREVIEW_EXT {
                        let mut new_name = preview_file.clone();
                        new_name.set_extension(PREVIEW_EXT);
                        fs::rename(preview_file, new_name).await?;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn save_info(info_file: &Path, info: &Value) -> anyhow::Result<()> {
    if !info_file.extension().unwrap_or_default().eq("json") {
        return Err(anyhow::anyhow!("Invalid json extension. Do you save to wrong file?"));
    }
    let mut saved_file = File::create(info_file)?;
    let info_str = to_string_pretty(info)?;
    saved_file
        .write_all(info_str.as_bytes())
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok(())
}

pub fn calculate_blake3(file_path: &Path) -> std::io::Result<String> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(result.to_hex().to_string().to_lowercase())
}

fn generate_video_thumbnail(file_path: &Path, overwrite: bool) -> anyhow::Result<()> {
    let mut thumbnail_path = PathBuf::from(file_path);
    thumbnail_path.set_extension(PREVIEW_EXT);
    if !overwrite && thumbnail_path.exists() {
        return Ok(());
    }

    Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "quiet",
            "-i",
            file_path.to_str().unwrap_or_default(),
            "-frames",
            "1",
            "-vf",
            r#"select=not(mod(n\,3000)),scale=300:ih*300/iw"#,
            "-q:v",
            "10",
            thumbnail_path.to_str().unwrap_or_default(),
        ])
        .status()?;

    Ok(())
}

pub async fn file_type(path: &Path) -> FileType {
    let data = fs::read(path).await.ok().unwrap_or_default();
    if let Some(kind) = infer::get(&data) {
        if kind.mime_type().starts_with("video/") {
            return FileType::Video;
        } else if kind.mime_type().starts_with("image/") {
            return FileType::Image;
        }
    }

    FileType::NA
}

pub fn get_extension_from_url(url: &str) -> Option<String> {
    url.split('/')
        .next_back()
        .and_then(|filename| Path::new(filename).extension())
        .and_then(|ext| ext.to_str().map(|ext| ext.to_string()))
}
