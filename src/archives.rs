use actix_web::Error;
use crate::folders::FolderData;

pub fn get_archive_data(absolute_path: String, changed_since: Option<u128>) -> Result<FolderData, Error> {
    todo!("Implement get_archive_data");
}