use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
struct CurseFile {
    #[serde(rename = "fileID")]
    file_id: u32,
    #[serde(rename = "projectID")]
    project_id: u32,
    required: bool,
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
