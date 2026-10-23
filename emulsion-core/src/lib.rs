pub mod colorops;
pub mod decode;
pub mod demosaic;
pub mod error;
pub mod export;
pub mod film;
pub mod geometry;
pub mod image;
pub mod params;
pub mod tonemap;

use error::Result;
use image::RgbaImage;
use params::PipelineParams;

#[cfg(test)]
pub mod tests;

/// The whole engine: one pure function.
/// `source` is a file path; the result is a finished RGBA8 image.
pub fn process(source: &str, params: &PipelineParams) -> Result<RgbaImage> {
    let base = decode::decode(source)?;
    Ok(tonemap::develop(&base, params))
}

// Re-export the public data types at the crate root.
pub use image::LinearImage;
pub use params::CropRect;
