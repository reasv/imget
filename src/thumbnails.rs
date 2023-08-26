use std::path::PathBuf;

use actix_web::Error;
use image::DynamicImage;

use crate::error::ImgetError;

pub fn generate_thumbnail(img: DynamicImage, max_h: u32, max_w: u32, thumb_path: &PathBuf, hq: Option<bool>) -> Result<(), Error> {
    // Option for fast thumbnail method
    let thumb = match hq {
        Some(true) => img.resize(max_w, max_h, image::imageops::FilterType::Lanczos3), 
        _ => img.thumbnail(max_w, max_h)
    };

    thumb.save_with_format(&thumb_path, image::ImageFormat::Jpeg)
        .map_err(|e| ImgetError::from(e))?;
    Ok(())
}