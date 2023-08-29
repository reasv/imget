
use actix_web::{Error, http::StatusCode};
use std::{path, time::UNIX_EPOCH, os::windows::prelude::MetadataExt};
use serde::Serialize;
use std::fs;

use crate::error::ImgetError;

#[derive(Serialize)]
pub struct FileEntry {
    pub name: String,
    pub is_directory: bool,
    pub last_modified: u128,
    pub fsize: u64,
    pub absolute_path: String,
    pub parent_path: String,
}
#[derive(Serialize)]
pub struct FolderData {
    pub entries: Vec<FileEntry>,
    pub absolute_path: String,
    pub parent_path: Option<String>,
}

pub fn get_folder_data(absolute_path: String, changed_since: Option<u128>) -> Result<FolderData, Error> {
    let entries = fs::read_dir(&absolute_path)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let metadata = entry.metadata().ok()?;
            let is_directory = metadata.is_dir();
            let name = entry.file_name().into_string().ok()?;
            let fsize = metadata.file_size();
            let last_modified = metadata.modified().ok()?
                .duration_since(UNIX_EPOCH).ok()?
                .as_millis();

            if let Some(changed_since) = changed_since {
                if last_modified <= changed_since {
                    return None
                }
            }
            let canonical_path = get_canonical_path(path::Path::new(&absolute_path).join(&name).to_str()?).ok()?;
            Some(FileEntry {
                name,
                is_directory,
                last_modified,
                fsize,
                absolute_path: canonical_path,
                parent_path: String::from(&absolute_path)
            })
        })
        .collect::<Vec<_>>();
    
    let parent_path = path::Path::new(&absolute_path).parent()
        .filter(|p| p.to_str().is_some())
        .map(|p| String::from(p.to_str().unwrap_or("")));

    let folder_data = FolderData {entries, absolute_path, parent_path};
    return Ok(folder_data);
}

pub fn get_canonical_path(path: &str) -> Result<String, Error> {
    let absolute_path = String::from(fs::canonicalize(path)?.to_str()
        .ok_or(ImgetError { message: String::from(format!("Path not found: {}", path)), status_code: StatusCode::NOT_FOUND})?);
    Ok(absolute_path)
}