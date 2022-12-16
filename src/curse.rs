use std::{io, path::Path};

use async_zip::read::seek::ZipFileReader;
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
struct CurseFile {
    #[serde(rename = "fileID")]
    file_id: u32,
    #[serde(rename = "projectID")]
    project_id: u32,
    required: bool,
}

impl CurseFile {
    async fn get_info(
        &self,
        client: &Client,
        api_key: &str,
    ) -> crate::error::Result<serde_json::Value> {
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
        Ok(response)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseManifest {
    files: Vec<CurseFile>,
    manifest_type: String,
    manifest_version: u8,
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
}
