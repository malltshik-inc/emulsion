/// Finished 8-bit image, RGBA interleaved.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RgbaImage {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl RgbaImage {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0_u8; width * height * 4],
        }
    }
}

/// Working image: one flat f32 buffer, RGB interleaved (stride = 3),
/// values in linear light 0.0..1.0. A single interleaved buffer keeps every op
/// safely parallelizable with rayon: we iterate chunks of the buffer we write.
#[derive(Clone, PartialEq, Debug)]
pub struct LinearImage {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl LinearImage {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0.0_f32; width * height * 3],
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> (f32, f32, f32) {
        let i = (y * self.width + x) * 3;
        (self.data[i], self.data[i + 1], self.data[i + 2])
    }

    pub fn set_px(&mut self, x: usize, y: usize, r: f32, g: f32, b: f32) {
        let i = (y * self.width + x) * 3;
        self.data[i] = r;
        self.data[i + 1] = g;
        self.data[i + 2] = b;
    }
}
