use std::{
    io,
    path::{Path, PathBuf},
};

use async_zip::read::seek::ZipFileReader;
use futures::{stream, StreamExt};
use lazy_static::lazy_static;
use reqwest::Client;
use serde::Deserialize;

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
        const BASE_CURSE_URL: &str = "https://api.curseforge.com";
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

    pub async fn install_to(&self, target: &Path) -> crate::error::Result<()> {
        if !target.is_dir() {
            return Err(
                io::Error::new(io::ErrorKind::NotFound, "target is not a directory").into(),
            );
        }
        let num_cpus = num_cpus::get();
        let client = Client::new();
        let api_key = std::env::var("CURSE_API_KEY").expect("CURSE_API_KEY not set");
        stream::iter(&self.manifest.files)
            .map(|file| {
                let client = &client;
                let api_key = &api_key;
                async move {
                    let info = file.get_info(client, api_key).await?;
                    Ok::<_, crate::error::Error>(info)
                }
            })
            .buffer_unordered(num_cpus * 2)
            .for_each_concurrent(num_cpus, |info| {
                let target = &target;
                let client = &client;
                async move {
                    match info {
                        Ok(info) => {
                            match info.download(client, target).await {
                                Ok(_) => {
                                    // log success
                                }
                                Err(e) => {
                                    // log error
                                    eprintln!("Failed to download {}", info.file_name);
                                    eprintln!("{:?}", e);
                                }
                            }
                            // log success
                        }
                        Err(e) => {
                            // log error
                            eprintln!("{:?}", e);
                        }
                    }
                }
            })
            .await;

        Ok(())
    }
}
