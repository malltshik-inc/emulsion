use std::io::{BufWriter, Write};

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use serde::Serialize;

use crate::{
    error::{CoreError, Result},
    image::RgbaImage,
    params::PipelineParams,
};

fn dynamic_image(w: u32, h: u32, data: Vec<u8>) -> Result<DynamicImage> {
    let buf: Option<ImageBuffer<Rgba<u8>, Vec<u8>>> = ImageBuffer::from_raw(w, h, data);
    match buf {
        Some(image) => Ok(DynamicImage::ImageRgba8(image)),
        None => Err(CoreError::Internal("image buffer allocation failed".into())),
    }
}

/// Save as PNG (lossless).
pub fn save_png(path: &str, img: &RgbaImage) -> Result<()> {
    let dimg = dynamic_image(img.width as u32, img.height as u32, img.data.clone())?;
    dimg.save_with_format(path, ImageFormat::Png)
        .map_err(|e| CoreError::Io(format!("png: {}", e)))?;
    Ok(())
}

/// Sidecar written next to an export: the exact settings used to develop it,
/// so a conversion can be reproduced and analysed later.
#[derive(Serialize)]
struct Sidecar<'a> {
    format: &'static str,
    source: &'a str,
    params: &'a PipelineParams,
}

pub fn save_sidecar(path: &str, source: &str, params: &PipelineParams) -> Result<()> {
    let sidecar = Sidecar {
        format: "emulsion-params-v1",
        source,
        params,
    };
    let json = serde_json::to_string_pretty(&sidecar)
        .map_err(|e| CoreError::Internal(format!("sidecar json: {}", e)))?;
    std::fs::write(path, json)
        .map_err(|e| CoreError::Io(format!("cannot create {}: {}", path, e)))?;
    Ok(())
}

/// Save as JPEG with the given quality (1..100).
pub fn save_jpeg(path: &str, img: &RgbaImage, quality: u8) -> Result<()> {
    let dimg = dynamic_image(img.width as u32, img.height as u32, img.data.clone())?;
    let mut bytes: Vec<u8> = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality);
    dimg.write_with_encoder(encoder)
        .map_err(|e| CoreError::Io(format!("jpeg: {}", e)))?;
    let f = std::fs::File::create(path)
        .map_err(|e| CoreError::Io(format!("cannot create {}: {}", path, e).into()))?;
    let mut buf = BufWriter::new(f);
    buf.write_all(&bytes)
        .map_err(|e| CoreError::Io(format!("write: {}", e)))?;
    Ok(())
}
