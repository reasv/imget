use actix_files::NamedFile;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, get};
use actix_web_actors::ws;
use folders::FolderData;
use std::{path::{PathBuf, self}, fs::Metadata};
use tokio::sync::mpsc;
use serde::Deserialize;
use std::fs;
use actix_cors::Cors;

mod change_watcher;
mod archives;
mod folders;
mod utils;
mod thumbnails;
mod error;
use error::ImgetError;

#[derive(Deserialize)]
struct FolderRequestParam {
    path: String,
    changed_since: Option<u128>,
    archive_access: Option<bool>,
    flatten_archives: Option<bool>
}

#[get("/folder")]
async fn get_folder(web::Query(params): web::Query<FolderRequestParam>) -> Result<HttpResponse, Error> {
    let directory = params.path;
    let abs_path_res = folders::get_canonical_path(&directory);
    
    if abs_path_res.is_err() {
        if let Some((archive_path, inner_path)) = archives::split_archive_path(PathBuf::from(directory)) {

            let fdata_raw = archives::get_archive_data(archive_path.to_str().unwrap().to_string(), None).unwrap();
            let folder_data = archives::get_archive_subfolder(fdata_raw, inner_path.to_str().unwrap().to_string()).unwrap();

            return Ok(HttpResponse::Ok().json(folder_data))
        }
        return Ok(HttpResponse::NotFound().finish());
    }

    let absolute_path = abs_path_res?;
    let path_metadata = fs::metadata(&absolute_path)?;
    let archive_access = params.archive_access.unwrap_or(false);
    let flatten_archives = params.flatten_archives.unwrap_or(false);
    
    let mut folder_data;
    if !path_metadata.is_dir() && archive_access && is_archive(path_metadata, &absolute_path) {
        let mut folder_data_flat = archives::get_archive_data(absolute_path, params.changed_since)?;
        if flatten_archives {
            // Cannot use directories if flattened
            folder_data_flat.entries.retain(|e| !e.is_directory);
            folder_data = folder_data_flat;
        } else {
            folder_data = archives::get_archive_subfolder(folder_data_flat, "".to_string())?;
        }
    } else {
        folder_data = folders::get_folder_data(absolute_path, params.changed_since)?;
        if archive_access {
            folder_data = archive_as_folder(folder_data);
        }
    }

    Ok(HttpResponse::Ok().json(folder_data))
}

fn is_archive(metadata: Metadata, path: &str) -> bool {
    let extension = path::Path::new(path).extension();
    if let Some(extension) = extension {
        let extension = extension.to_str().unwrap_or("");
        return metadata.is_file() && (extension == "zip" || extension == "rar" || extension == "7z");
    }
    false
}

fn archive_as_folder(mut folder_data: FolderData) -> FolderData {
    let archive_extensions: Vec<&str> = vec!["zip", "rar", "7z"];
    for entry in folder_data.entries.iter_mut() {
        if !entry.is_directory {
            let extension = path::Path::new(&entry.name).extension().unwrap_or_default().to_str().unwrap_or("");
            if archive_extensions.contains(&extension) {
                // Treat archive files as folders
                entry.is_directory = true;
            }
        }
    }
    return folder_data;
}

#[derive(Deserialize)]
struct FileRequestParam {
    path: String,
}

#[get("/file")]
async fn static_files(web::Query(params): web::Query<FileRequestParam>, req: HttpRequest) -> Result<HttpResponse, Error> {
    let path: PathBuf = PathBuf::from(params.path);
    match NamedFile::open_async(&path).await {
        Ok(file) => Ok(file.into_response(&req)),
        Err(e) => {
            if let Some(buf) = archives::try_archive_file(path) {
                return Ok(HttpResponse::Ok().body(buf));
            }
            Err(e.into())
        }
    }
}

#[derive(Deserialize)]
pub struct ThumbnailRequestParam {
    path: String,
    hq: Option<bool>,
    max_w: Option<u32>,
    max_h: Option<u32>
}

#[get("/thumbnail")]
async fn get_thumbnail(web::Query(params): web::Query<ThumbnailRequestParam>) -> Result<NamedFile, Error> {
    let path: PathBuf = PathBuf::from(params.path);
    let max_w = params.max_w.unwrap_or(512);
    let max_h = params.max_h.unwrap_or(512);

    let img = image::open(path)
        .map_err(|e| ImgetError::from(e))?;

    let hash = utils::hash_u8_array(img.as_bytes());

    let thumb_path = PathBuf::from(format!("./thumbs/{}-w{}h{}-hq-{}.jpeg", hash, max_w, max_h, params.hq.unwrap_or(false)));

    match NamedFile::open_async(&thumb_path).await {
        Ok(file) => Ok(file),
        Err(_) => {
            thumbnails::generate_thumbnail(img, max_h, max_w, &thumb_path, params.hq)?;
            let file = NamedFile::open_async(thumb_path).await?;
            Ok(file)
        }
    }
}

#[get("/ws/watch")]
async fn watch_folder(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let (tx, rx) = mpsc::unbounded_channel();
    // Get a debouncer over notify to watch changes
    let debouncer = change_watcher::get_folder_watcher(tx);
    // Upgrade the connection to a WebSocket
    let resp: HttpResponse = ws::start(change_watcher::WatcherWsActor { rx, debouncer }, &req, stream)?;
    Ok(resp)
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        let cors = Cors::permissive();
        App::new()
            .wrap(cors)
            .service(watch_folder)
            .service(static_files)
            .service(get_folder)
            .service(get_thumbnail)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
