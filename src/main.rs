use actix_files::NamedFile;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, get, http::StatusCode};
use actix_web_actors::ws;
use std::{path::{PathBuf, self}, time::UNIX_EPOCH, os::windows::prelude::MetadataExt};
use tokio::sync::mpsc;
use serde::{Serialize, Deserialize};
use std::fs;
use actix_cors::Cors;

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
    fsize: u64,
    absolute_path: String,
    parent_path: String,
}
#[derive(Serialize)]
struct FolderData {
    entries: Vec<FileEntry>,
    absolute_path: String,
    parent_path: Option<String>,
}
#[derive(Deserialize)]
struct FolderRequestParam {
    path: String,
    changed_since: Option<u128>
}

#[get("/folder")]
async fn get_folder(web::Query(params): web::Query<FolderRequestParam>) -> Result<HttpResponse, Error> {
    let directory = params.path;
    let absolute_path = get_canonical_path(&directory)?;

    let entries = fs::read_dir(&directory)?
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
            let canonical_path = get_canonical_path(path::Path::new(&directory).join(&name).to_str()?).ok()?;
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
    Ok(HttpResponse::Ok().json(folder_data))
}

fn get_canonical_path(path: &str) -> Result<String, Error> {
    let absolute_path = String::from(fs::canonicalize(path)?.to_str()
        .ok_or(ImgetError { message: String::from(format!("Path not found: {}", path)), status_code: StatusCode::NOT_FOUND})?);
    Ok(absolute_path)
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
