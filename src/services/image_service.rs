use crate::error::{AppError, AppResult};
use image::imageops::FilterType;
use image::io::Reader as ImageReader;
use image::ImageFormat;
use std::io::Cursor;
use std::path::Path;
use tracing::{debug, error};

pub struct ImageService;

impl ImageService {
    /// Generate a thumbnail for the image at the given path.
    /// Returns the raw bytes of the thumbnail (encoded as PNG).
    pub fn generate_thumbnail(path: &Path, max_width: u32, max_height: u32) -> AppResult<Vec<u8>> {
        debug!(
            "Generating thumbnail for {:?} ({:?}x{:?})",
            path, max_width, max_height
        );

        // Open and decode the image
        let img = ImageReader::open(path)
            .map_err(|e| AppError::InvalidInput(format!("Failed to open image file: {}", e)))?
            .with_guessed_format()
            .map_err(|e| AppError::InvalidInput(format!("Failed to guess image format: {}", e)))?
            .decode()
            .map_err(|e| {
                error!("Failed to decode image {:?}: {}", path, e);
                AppError::InternalError(format!("Failed to decode image: {}", e))
            })?;

        // Resize the image
        // use thumbnail() which is faster for downscaling
        let thumbnail = img.thumbnail(max_width, max_height);

        // Encode to buffer
        let mut buffer = Cursor::new(Vec::new());
        thumbnail
            .write_to(&mut buffer, ImageFormat::Png)
            .map_err(|e| {
                error!("Failed to encode thumbnail: {}", e);
                AppError::InternalError(format!("Failed to encode thumbnail: {}", e))
            })?;

        Ok(buffer.into_inner())
    }
}
