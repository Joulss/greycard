/// A picture read back from the device: `width` by `height`, a pixel
/// four eight-bit channels (red, green, blue, alpha), rows packed top
/// to bottom. The window makes Slint's picture of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rgba8 {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 4]>,
}

impl Rgba8 {
    /// From packed bytes, four a pixel; `None` when they are not
    /// `width` by `height` of them.
    pub fn from_bytes(width: u32, height: u32, bytes: &[u8]) -> Option<Self> {
        if bytes.len() != width as usize * height as usize * 4 {
            return None;
        }
        Some(Self {
            width,
            height,
            pixels: bytes.as_chunks::<4>().0.to_vec(),
        })
    }

    /// The pixels as packed bytes, four a pixel.
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::Rgba8;

    #[test]
    fn the_bytes_round_trip_and_a_wrong_count_is_refused() {
        let bytes: Vec<u8> = (0..2 * 3 * 4).map(|v| v as u8).collect();
        let picture = Rgba8::from_bytes(2, 3, &bytes).unwrap();
        assert_eq!(picture.pixels[1], [4, 5, 6, 7]);
        assert_eq!(picture.as_bytes(), bytes.as_slice());
        assert_eq!(Rgba8::from_bytes(2, 2, &bytes), None);
    }
}
