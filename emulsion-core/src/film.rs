//! Bundled film simulations (Hald CLUTs).
//!
//! Every LUT is a level-6 Hald image (216×216, 36 levels per channel) stored as
//! RGB PNG. A Hald image is a 3D cube flattened in red-fastest order, so the
//! decoded pixel array can be indexed directly as `(b * levels + g) * levels + r`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::error::{CoreError, Result};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    BlackAndWhite,
    Color,
}

pub struct Film {
    pub name: &'static str,
    pub category: Category,
    pub png: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/film_registry.rs"));

/// All bundled simulations, black-and-white first, then colour.
pub fn films() -> &'static [Film] {
    FILMS
}

pub fn find(id: u16) -> Option<&'static Film> {
    FILMS.get(id as usize)
}

pub fn name(id: u16) -> Option<&'static str> {
    find(id).map(|film| film.name)
}

/// A decoded Hald CLUT: an RGB cube with `levels` samples per channel.
pub struct HaldLut {
    levels: usize,
    data: Vec<u8>,
}

impl HaldLut {
    pub fn from_png(bytes: &[u8]) -> Result<Self> {
        let image = image::load_from_memory(bytes)
            .map_err(|e| CoreError::Unsupported(format!("film lut: {}", e)))?;
        let image = image.to_rgb8();
        Self::from_rgb(image.width() as usize, image.into_raw())
    }

    /// Build from a square, RGB, red-fastest Hald image.
    pub fn from_rgb(side: usize, data: Vec<u8>) -> Result<Self> {
        if side == 0 || data.len() != side * side * 3 {
            return Err(CoreError::Unsupported(format!(
                "film lut: bad buffer {} for side {}",
                data.len(),
                side
            )));
        }
        let level = (side as f64).cbrt().round() as usize;
        if level * level * level != side {
            return Err(CoreError::Unsupported(format!(
                "film lut: side {} is not a Hald level cube",
                side
            )));
        }
        let levels = level * level;
        if data.len() != levels * levels * levels * 3 {
            return Err(CoreError::Unsupported(
                "film lut: inconsistent levels".into(),
            ));
        }
        Ok(Self { levels, data })
    }

    pub fn levels(&self) -> usize {
        self.levels
    }

    /// Sample the cube with trilinear interpolation. Input and output are
    /// sRGB-encoded values in `0..=1`.
    pub fn sample(&self, rgb: [f32; 3]) -> [f32; 3] {
        let max = (self.levels - 1) as f32;
        let r = rgb[0].clamp(0.0, 1.0) * max;
        let g = rgb[1].clamp(0.0, 1.0) * max;
        let b = rgb[2].clamp(0.0, 1.0) * max;
        let r0 = r as usize;
        let g0 = g as usize;
        let b0 = b as usize;
        let r1 = (r0 + 1).min(self.levels - 1);
        let g1 = (g0 + 1).min(self.levels - 1);
        let b1 = (b0 + 1).min(self.levels - 1);
        let (fr, fg, fb) = (r - r0 as f32, g - g0 as f32, b - b0 as f32);

        let at = |ri: usize, gi: usize, bi: usize| -> [f32; 3] {
            let i = (ri + self.levels * (gi + self.levels * bi)) * 3;
            [
                self.data[i] as f32 / 255.0,
                self.data[i + 1] as f32 / 255.0,
                self.data[i + 2] as f32 / 255.0,
            ]
        };

        let c000 = at(r0, g0, b0);
        let c100 = at(r1, g0, b0);
        let c010 = at(r0, g1, b0);
        let c110 = at(r1, g1, b0);
        let c001 = at(r0, g0, b1);
        let c101 = at(r1, g0, b1);
        let c011 = at(r0, g1, b1);
        let c111 = at(r1, g1, b1);

        let mut out = [0.0_f32; 3];
        for (channel, value) in out.iter_mut().enumerate() {
            let c00 = c000[channel] + (c100[channel] - c000[channel]) * fr;
            let c01 = c001[channel] + (c101[channel] - c001[channel]) * fr;
            let c10 = c010[channel] + (c110[channel] - c010[channel]) * fr;
            let c11 = c011[channel] + (c111[channel] - c011[channel]) * fr;
            let c0 = c00 + (c10 - c00) * fg;
            let c1 = c01 + (c11 - c01) * fg;
            *value = c0 + (c1 - c0) * fb;
        }
        out
    }
}

static CACHE: OnceLock<Mutex<HashMap<u16, Arc<HaldLut>>>> = OnceLock::new();

/// Decode a bundled simulation once, then reuse it. The lock is held across
/// the (slow) decode so two threads can't decode the same LUT concurrently.
pub fn lut(id: u16) -> Result<Arc<HaldLut>> {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(lut) = cache.get(&id) {
        return Ok(Arc::clone(lut));
    }
    let film = find(id)
        .ok_or_else(|| CoreError::Unsupported(format!("film lut: no simulation with id {}", id)))?;
    let lut = Arc::new(HaldLut::from_png(film.png)?);
    Ok(Arc::clone(cache.entry(id).or_insert(lut)))
}
