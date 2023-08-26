use actix_files::NamedFile;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, get};
use actix_web_actors::ws;
use std::{path::PathBuf, time::{SystemTime, UNIX_EPOCH}};
use tokio::sync::mpsc;

mod change_watcher;

use serde::{Serialize, Deserialize};
use std::fs::{self, Metadata};

#[derive(Serialize)]
struct FileEntry {
    name: String,
    is_directory: bool,
    last_modified: u128
}

#[derive(Deserialize)]
struct FileRequestParam {
    directory: String,
}

#[get("/files")]
async fn get_files(web::Query(params): web::Query<FileRequestParam>) -> Result<HttpResponse, Error> {
    let directory = params.directory;
    let entries = fs::read_dir(directory)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let metadata = entry.metadata().ok()?;
            let is_directory = metadata.is_dir();
            let name = entry.file_name().into_string().ok()?;
            let last_modified = metadata.modified().ok()?
                .duration_since(UNIX_EPOCH).ok()?
                .as_millis();

            Some(FileEntry {
                name,
                is_directory,
                last_modified
            })
        })
        .collect::<Vec<_>>();

    Ok(HttpResponse::Ok().json(entries))
}

#[get("/ws/watch")]
async fn watch(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let (tx, rx) = mpsc::unbounded_channel();
    // Get a debouncer over notify to watch changes
    let debouncer = change_watcher::get_folder_watcher(tx);
    // Upgrade the connection to a WebSocket
    let resp: HttpResponse = ws::start(change_watcher::WatcherWsActor { rx, debouncer }, &req, stream)?;
    Ok(resp)
}

#[get("/file/{filename:.*}")]
async fn static_files(req: HttpRequest) -> Result<NamedFile, Error> {
    let path: PathBuf = req.match_info().query("filename").parse()?;
    let file = NamedFile::open(path)?;
    Ok(file)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(watch)
            .service(static_files)
            .service(get_files)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
