use actix_files::NamedFile;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, get};
use actix_web_actors::ws;
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
    archive_access: Option<bool>
}

#[get("/folder")]
async fn get_folder(web::Query(params): web::Query<FolderRequestParam>) -> Result<HttpResponse, Error> {
    let directory = params.path;
    let absolute_path = folders::get_canonical_path(&directory)?;
    let path_metadata = fs::metadata(&absolute_path)?;
    
    let folder_data;
    if !path_metadata.is_dir() && params.archive_access.is_some_and(|v| v) && is_archive(path_metadata, &absolute_path) {
        folder_data = archives::get_archive_data(absolute_path, params.changed_since)?;
        
    } else {
        folder_data = folders::get_folder_data(absolute_path, params.changed_since)?;
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
