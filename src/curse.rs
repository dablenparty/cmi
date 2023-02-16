use std::{
    fmt, io,
    path::{Path, PathBuf},
};

use async_zip::read::seek::ZipFileReader;
use futures::{stream, StreamExt};
use lazy_static::lazy_static;
use log::{debug, error, info};
use reqwest::Client;
use serde::Deserialize;

use crate::util::sanitize_zip_filename;

const BASE_CURSE_URL: &str = "https://api.curseforge.com";

#[derive(Debug, Clone, Copy, Deserialize)]
struct CurseFile {
    #[serde(rename = "fileID")]
    file_id: u32,
    #[serde(rename = "projectID")]
    project_id: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseFileInfo {
    display_name: String,
    download_url: Option<String>,
    file_name: String,
}

impl CurseFileInfo {
    async fn download(&self, client: &Client, folder: &Path) -> crate::error::Result<PathBuf> {
        lazy_static! {
            static ref ILLEGAL_CHARS: regex::Regex = regex::Regex::new(r#"[\\/:*?"<>|]"#)
                .expect("Failed to compile ILLEGAL_CHARS regex");
        }
        debug!("Downloading {}", self.display_name);
        let parent_folder = if self.file_name.ends_with("zip") {
            "resourcepacks"
        } else {
            "mods"
        };
        let target = folder.join(parent_folder);
        dablenutil::tokio::async_create_dir_if_not_exists(&target).await?;
        let file_name = ILLEGAL_CHARS.replace_all(&self.file_name, "").to_string();
        if self.download_url.is_none() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "download_url not found").into());
        }
        let download_url = self.download_url.as_ref().unwrap().replace('"', "");
        let path = target.join(file_name);
        if path.exists() {
            return Ok(path);
        }
        let mut file_handle = tokio::fs::File::create(&path).await?;
        let response = client.get(&download_url).send().await?.error_for_status()?;
        let content = response.bytes().await?;
        tokio::io::copy(&mut content.to_vec().as_slice(), &mut file_handle).await?;
        Ok(path)
    }
}

impl CurseFile {
    async fn get_info(
        &self,
        client: &Client,
        api_key: &str,
    ) -> crate::error::Result<CurseFileInfo> {
        let endpoint = format!("/v1/mods/{}/files/{}", self.project_id, self.file_id);
        let url = format!("{}{}", BASE_CURSE_URL, endpoint);
        let response = client
            .get(&url)
            .header("Accept", "application/json")
            .header("x-api-key", api_key)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        let data = response.get("data").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "data not found in curseforge response",
            )
        })?;
        let info: CurseFileInfo = serde_json::from_value(data.clone())?;
        Ok(info)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseManifest {
    files: Vec<CurseFile>,
    name: String,
    overrides: String,
    version: String,
}

pub struct CurseModpack {
    manifest: CurseManifest,
    archive: ZipFileReader<tokio::fs::File>,
}

impl fmt::Display for CurseModpack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.manifest.name, self.manifest.version)
    }
}

impl CurseModpack {
    pub async fn load(path: &Path) -> crate::error::Result<Self> {
        let file = tokio::fs::File::open(path).await?;
        let mut archive = ZipFileReader::new(file).await?;
        for (i, entry) in archive.entries().iter().enumerate() {
            if entry.filename() == "manifest.json" {
                let reader = archive.entry_reader(i).await?;
                let text = reader.read_to_string_crc().await?;
                let manifest: CurseManifest = serde_json::from_str(&text)?;
                return Ok(Self { manifest, archive });
            }
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "manifest.json not found").into())
    }

    async fn copy_overrides(&mut self, target: &Path) -> crate::error::Result<()> {
        info!("Copying overrides...");
        let entry_count = self.archive.entries().len();
        let mut overrides_count = 0;
        for i in 0..entry_count {
            let mut entry_reader = self.archive.entry_reader(i).await?;
            let entry = entry_reader.entry();
            let entry_name = entry.filename();
            let file_name = sanitize_zip_filename(entry_name)
                .unwrap_or_else(|| panic!("Invalid filename: {}", entry_name));
            // ensure that the file is in the overrides folder and not a directory
            if !file_name.starts_with(&self.manifest.overrides)
                || entry_name.ends_with('/')
                || entry_name.ends_with('\\')
            {
                continue;
            }
            let file_name = file_name.strip_prefix(&self.manifest.overrides).unwrap();
            let target_path = target.join(file_name);
            if target_path.exists() {
                debug!("{} already exists, skipping", target_path.display());
                continue;
            }
            let parent = target_path.parent().unwrap();
            debug!("Copying {} to {}", entry_name, target_path.display());
            dablenutil::tokio::async_create_dir_if_not_exists(parent).await?;
            let mut file_handle = tokio::fs::File::create(&target_path).await?;
            tokio::io::copy(&mut entry_reader, &mut file_handle).await?;
            overrides_count += 1;
        }
        info!("Copied {} overrides", overrides_count);
        Ok(())
    }

    pub async fn install_to(&mut self, target: &Path) -> crate::error::Result<()> {
        if !target.is_dir() {
            return Err(
                io::Error::new(io::ErrorKind::NotFound, "target is not a directory").into(),
            );
        }
        info!(
            "Beginning install of {} to {}",
            self.manifest.name,
            target.display()
        );
        let num_cpus = num_cpus::get();
        // collect file id's into json array
        let file_ids: Vec<_> = self
            .manifest
            .files
            .iter()
            .map(|file| file.file_id)
            .collect();
        let file_ids = serde_json::to_string(&file_ids)?;
        let body = format!("{{\"fileIds\":{}}}", file_ids);
        let client = Client::new();
        let api_key = std::env::var("CURSE_API_KEY").expect("CURSE_API_KEY not set");
        info!("Downloading {} files", self.manifest.files.len());
        let url = format!("{}/v1/mods/files", BASE_CURSE_URL);
        let response = client
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("x-api-key", api_key)
            .body(body)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        let file_infos = response
            .get("data")
            .map(|data| serde_json::from_value::<Vec<CurseFileInfo>>(data.clone()))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "data not found in curseforge response",
                )
            })??;
        stream::iter(file_infos)
            .for_each_concurrent(num_cpus, |info| {
                let target = &target;
                let client = &client;
                async move {
                    match info.download(client, target).await {
                        Ok(p) => {
                            debug!("{} downloaded to {}", info.file_name, p.display());
                        }
                        Err(e) => {
                            if let crate::error::Error::IoError(e) = e {
                                if e.kind() == io::ErrorKind::NotFound {
                                    error!(
                                        "Failed to download {}, no download URL found",
                                        info.file_name
                                    );
                                } else {
                                    error!("Failed to download {}", info.file_name);
                                    error!("{:?}", e);
                                }
                            } else {
                                error!("Failed to download {}", info.file_name);
                                error!("{:?}", e);
                            }
                        }
                    }
                }
            })
            .await;
        self.copy_overrides(target).await?;
        Ok(())
    }
}
