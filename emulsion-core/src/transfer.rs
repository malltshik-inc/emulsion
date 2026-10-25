//! sRGB transfer functions, shared by decode and tonemap.

const SRGB_CROSSOVER: f32 = 0.00304;
const LINEAR_GAIN: f32 = 12.92;
const SRGB_GAIN: f32 = 1.055;
const SRGB_OFFSET: f32 = 0.055;
const SRGB_GAMMA: f32 = 2.4;

/// Look-up table size per direction. 65536 entries make the error < 2e-4 in
/// the encoded domain — far below one output level, so byte output is stable.
const LUT_SIZE: usize = 65536;

fn make_encode_lut() -> Vec<f32> {
    (0..LUT_SIZE)
        .map(|i| {
            let v = i as f32 / (LUT_SIZE - 1) as f32;
            if v <= SRGB_CROSSOVER {
                v * LINEAR_GAIN
            } else {
                v.powf(1.0 / SRGB_GAMMA) * SRGB_GAIN - SRGB_OFFSET
            }
        })
        .collect()
}

fn make_decode_lut() -> Vec<f32> {
    (0..LUT_SIZE)
        .map(|i| {
            let v = i as f32 / (LUT_SIZE - 1) as f32;
            if v <= 0.04045 {
                v / LINEAR_GAIN
            } else {
                ((v + SRGB_OFFSET) / SRGB_GAIN).powf(SRGB_GAMMA)
            }
        })
        .collect()
}

static ENCODE: std::sync::LazyLock<Vec<f32>> = std::sync::LazyLock::new(make_encode_lut);
static DECODE: std::sync::LazyLock<Vec<f32>> = std::sync::LazyLock::new(make_decode_lut);

/// Table index for an encoded-domain value (clamped to `[0,1]`).
pub(crate) fn encode_index(v: f32) -> usize {
    (v.clamp(0.0, 1.0) * (LUT_SIZE - 1) as f32).round() as usize
}

/// Table index for a linear-domain value (clamped to `[0,1]`).
pub(crate) fn decode_index(v: f32) -> usize {
    (v.clamp(0.0, 1.0) * (LUT_SIZE - 1) as f32).round() as usize
}

/// Grab once before a hot loop and index directly (no per-call LazyLock).
pub(crate) fn encode_table() -> &'static [f32] {
    &ENCODE
}

pub(crate) fn decode_table() -> &'static [f32] {
    &DECODE
}
