use actix_web::{http::StatusCode, ResponseError, HttpResponse};
use image::ImageError;



#[derive(Debug)]
pub struct ImgetError {
    pub message: String,
    pub status_code: StatusCode
}

impl std::fmt::Display for ImgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::convert::From<notify::Error> for ImgetError {
    fn from(err: notify::Error) -> Self {
        return ImgetError { message: format!("Notify Error: {}", err), status_code: StatusCode::INTERNAL_SERVER_ERROR }
    }
}

impl std::convert::From<ImageError> for ImgetError {
    fn from(err: ImageError) -> Self {
        ImgetError { status_code: StatusCode::INTERNAL_SERVER_ERROR, message: format!("Could not save thumbnail: {}", err)}
    }
}

impl ResponseError for ImgetError {
    fn status_code(&self) -> StatusCode {
        self.status_code
    }
    fn error_response(&self) -> HttpResponse {
        // Customize the HTTP response for your custom error
        HttpResponse::InternalServerError().body(self.message.clone())
    }
}

impl std::error::Error for ImgetError {}
