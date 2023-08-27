use actix_files::NamedFile;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, get};
use actix_web_actors::ws;
use std::{path::PathBuf, time::UNIX_EPOCH, os::windows::prelude::MetadataExt};
use tokio::sync::mpsc;
use serde::{Serialize, Deserialize};
use std::fs;

mod change_watcher;
mod utils;
mod thumbnails;
mod error;
use error::ImgetError;

#[derive(Serialize)]
struct FileEntry {
    name: String,
    is_directory: bool,
    last_modified: u128,
    fsize: u64
}

#[derive(Deserialize)]
struct FolderRequestParam {
    path: String,
    changed_since: Option<u128>
}

#[get("/folder")]
async fn get_folder(web::Query(params): web::Query<FolderRequestParam>) -> Result<HttpResponse, Error> {
    let directory = params.path;

    let entries = fs::read_dir(directory)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let metadata = entry.metadata().ok()?;
            let is_directory = metadata.is_dir();
            let name = entry.file_name().into_string().ok()?;
            let fsize = metadata.file_size();
            let last_modified = metadata.modified().ok()?
                .duration_since(UNIX_EPOCH).ok()?
                .as_millis();
            
            if let Some(changed_since) = params.changed_since {
                if last_modified <= changed_since {
                    return None
                }
            }
            Some(FileEntry {
                name,
                is_directory,
                last_modified,
                fsize
            })
        })
        .collect::<Vec<_>>();

    Ok(HttpResponse::Ok().json(entries))
}

#[derive(Deserialize)]
struct FileRequestParam {
    path: String,
}

#[get("/file")]
async fn static_files(web::Query(params): web::Query<FileRequestParam>) -> Result<NamedFile, Error> {
    let path: PathBuf = PathBuf::from(params.path);
    let file = NamedFile::open(path)?;
    Ok(file)
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

    match NamedFile::open(&thumb_path) {
        Ok(file) => Ok(file),
        Err(_) => {
            thumbnails::generate_thumbnail(img, max_h, max_w, &thumb_path, params.hq)?;
            let file = NamedFile::open(thumb_path)?;
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
        App::new()
            .service(watch_folder)
            .service(static_files)
            .service(get_folder)
            .service(get_thumbnail)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
