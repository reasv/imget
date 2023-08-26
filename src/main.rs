use actix_web::{get, web, App, HttpResponse, HttpServer, Error};
use serde::{Serialize, Deserialize};
use std::fs::{self, Metadata};

#[derive(Serialize)]
struct FileEntry {
    name: String,
    is_directory: bool,
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
            let entry = entry.unwrap();
            let metadata = entry.metadata().unwrap();
            let is_directory = metadata.is_dir();
            let name = entry.file_name().into_string().unwrap();
            Some(FileEntry {
                name,
                is_directory,
            })
        })
        .collect::<Vec<_>>();

    Ok(HttpResponse::Ok().json(entries))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new().service(get_files)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
