use actix_web::Error;
use unrar::{Archive, error::UnrarError};
use core::arch;
use std::path::{Path, PathBuf};
use crate::folders::{FolderData, FileEntry};

pub fn get_archive_data(absolute_path: String, changed_since: Option<u128>) -> Result<FolderData, Error> {
    let path_object = std::path::Path::new(&absolute_path);

    let archive = Archive::new(&absolute_path);
    let entries = archive.open_for_listing().unwrap()
    .filter_map(|entry| {
        let header = entry.ok()?;
        let modified = (header.file_time as u128) * 1000;
        if changed_since.is_some() && modified < changed_since.unwrap() {
            return None;
        }
        let name = String::from(header.filename.to_str()?);
        let size = header.unpacked_size;
        let is_dir = header.is_directory();
        let mut path_buf = path_object.to_path_buf();
        path_buf.push(header.filename);

        Some(FileEntry {
            name: name.clone(),
            fsize: size as u64,
            last_modified: modified,
            is_directory: is_dir,
            parent_path: absolute_path.clone(),
            absolute_path: path_buf.to_str()?.to_string(),
        })
    }).collect();

    Ok(FolderData {
        entries,
        absolute_path,
        parent_path: None,
    })
}

pub fn get_archive_subfolder(folder_data: FolderData, sub_folder: String) -> Result<FolderData, Error> {
    let archive_path_obj = Path::new(folder_data.absolute_path.as_str());

    let sub_folder_path = Path::new(&sub_folder);
    let sub_folder_path_abs = archive_path_obj.join(sub_folder_path);
    let parent_path = sub_folder_path_abs.parent().map(|p| p.to_str().unwrap_or("").to_string());

    let mut absolute_path = folder_data.absolute_path;

    let filtered_entries: Vec<FileEntry> = folder_data.entries.iter().filter_map(|entry|  {
        if entry.name == sub_folder {
            absolute_path = entry.absolute_path.clone();
        }
        // Is this entry in the subfolder?
        if (entry.name.starts_with(&sub_folder)) {
            let name_path = Path::new(&entry.name);
            let entry_parent = name_path.parent()?;
            println!("{:?}", entry_parent);
            // Is the subfolder the direct parent of this entry?
            if entry_parent.to_str()?.to_string() == sub_folder {
                let name = name_path.file_name()?.to_str()?.to_string();
                let parent_path = Path::new(&entry.absolute_path).parent()?.to_str()?.to_string();
                return Some(FileEntry { name, parent_path,
                    is_directory: entry.is_directory, last_modified: entry.last_modified, fsize: entry.fsize, absolute_path: entry.absolute_path.clone()});
            }
        }
        return None;
    }).collect();

    return Ok(FolderData { entries: filtered_entries, absolute_path, parent_path });
}

pub fn get_archive_file(archive_path: String, filename: String) -> Result<Option<Vec<u8>>, UnrarError> {
    let mut archive = Archive::new(&archive_path).open_for_processing()?;
    while let Some(header) = archive.read_header()? {
        archive = if header.entry().filename.as_os_str().to_str().unwrap_or("") == filename {
            let (data, rest) = header.read()?;
            drop(rest); // close the archive
            return Ok(Some(data));
        } else {
            header.skip()?
        }
    }
    Ok(None)
}

pub fn split_archive_path(path: PathBuf) -> Option<(PathBuf, PathBuf)> {
    let archive_extensions: Vec<&str> = vec!["zip", "rar", "7z"];
    if let Some(archive_path) = path.ancestors().find(|p| {
        let extension = p.extension().unwrap_or(std::ffi::OsStr::new(""));
        if archive_extensions.contains(&extension.to_str().unwrap_or("")) {
            if p.is_file() {
                return true;
            }
        }
        false
    }) {
        if let Ok(inner_path) = path.strip_prefix(archive_path) {
            return Some((archive_path.to_path_buf(), inner_path.to_path_buf()));
        }
    }
    return None
}

pub fn try_archive_file(full_path: PathBuf) -> Option<Vec<u8>> {
    let (archive_path, inner_path) = split_archive_path(full_path)?;
    let archive_path = archive_path.to_str()?;
    let inner_path = inner_path.to_str()?;
    let data = get_archive_file(archive_path.to_string(), inner_path.to_string()).ok()?;
    return data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        
        //println!("{:?}", fdata.entries[0]);
        // let data = get_archive_file("thumbs\\test.zip\\test.rar\\Rapiere.rar".to_string(), fdata.entries[0].name.to_string()).unwrap();
        //println!("{:?}", data.is_some());

        // split_archive_path(fdata.entries[0].absolute_path.clone());

        // println!("{}", try_archive_file(PathBuf::from(fdata.entries[0].absolute_path.clone()).to_path_buf()).is_some());
        let fullpath = "\\\\?\\Q:\\projects\\imget\\thumbs\\test.zip\\test.rar\\Rapiere.rar\\Rapiere CH 8 PNG".to_string();

        let (archive_path, inner_path) = split_archive_path(PathBuf::from(fullpath)).unwrap();

        let fdata = get_archive_data(archive_path.to_str().unwrap().to_string(), None).unwrap();

        println!("{:?}", get_archive_subfolder(fdata, inner_path.to_str().unwrap().to_string()).unwrap());

    }
}