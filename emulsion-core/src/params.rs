use serde::{Deserialize, Serialize};

/// A crop rectangle in fractional coordinates of the source (0.0..1.0).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct CropRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Everything the user can tweak. Plain data, serde-serializable (JSON patch).
/// Edits only change this struct and the pipeline re-runs.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct PipelineParams {
    /// WB temperature, -100..100 (0 = neutral, + = warmer)
    pub wb_temperature: f32,
    /// WB tint, -100..100 (0 = neutral, + = magenta)
    pub wb_tint: f32,
    /// Exposure in EV, -3.0..+3.0
    pub exposure: f32,
    /// Contrast, -100..100 (0 = neutral, + = punchier, - = flatter)
    pub contrast: f32,
    /// Saturation, -100..100 (0 = neutral, - = greyscale, + = more saturated)
    pub saturation: f32,
    /// Shadows, -100..100 (0 = neutral, + = lift/lower the dark tones)
    #[serde(default)]
    pub shadows: f32,
    /// Highlights, -100..100 (0 = neutral, + = raise/lower the bright tones)
    #[serde(default)]
    pub highlights: f32,
    /// Vignette, -100..100 (0 = none, + = darken edges, - = brighten edges)
    #[serde(default)]
    pub vignette: f32,
    /// Index into [`crate::film::films`], `None` = no film simulation.
    #[serde(default)]
    pub film: Option<u16>,
    /// Output boost applied only when Develop (invert) is on: simultaneously
    /// raises exposure, contrast and saturation. Range -100..100, 0 = none.
    #[serde(default)]
    pub invert_sensitivity: f32,
    /// Per-channel black point, 0.0..1.0
    pub black_point: [f32; 3],
    /// negative -> positive flip
    pub invert: bool,
    /// 90 degree rotations: 0..3
    pub rotate_90: u8,
    /// Fine angle in degrees, -5.0..+5.0
    pub fine_angle: f32,
    /// Mirror horizontally (left <-> right)
    #[serde(default)]
    pub flip_horizontal: bool,
    pub crop: Option<CropRect>,
}

impl Default for PipelineParams {
    fn default() -> Self {
        Self {
            wb_temperature: 0.0,
            wb_tint: 0.0,
            exposure: 0.0,
            contrast: 0.0,
            saturation: 0.0,
            shadows: 0.0,
            highlights: 0.0,
            vignette: 0.0,
            film: None,
            black_point: [0.0, 0.0, 0.0],
            invert: false,
            rotate_90: 0,
            fine_angle: 0.0,
            flip_horizontal: false,
            invert_sensitivity: 0.0,
            crop: None,
        }
    }
}
