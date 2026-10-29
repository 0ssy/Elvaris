//! Pixel and surface representation (Phase 18.2).

#![allow(dead_code)]

use alloc::vec::Vec;

use crate::heap;

/// Canonical pixel format: 4 bytes per pixel (RGBA32).
pub const BYTES_PER_PIXEL: usize = 4;

/// Surface errors.
#[derive(Debug, PartialEq, Eq)]
pub enum SurfaceError {
    InvalidDimensions,
    AllocationFailed,
    OutOfBounds,
    ArithmeticOverflow,
}

/// An off-screen pixel buffer.
#[derive(Clone, Debug)]
pub struct Surface {
    width: usize,
    height: usize,
    stride: usize,
    buffer: Vec<u8>,
}

impl Surface {
    pub fn new(width: usize, height: usize) -> Result<Self, SurfaceError> {
        if width == 0 || height == 0 {
            return Err(SurfaceError::InvalidDimensions);
        }

        let stride = width
            .checked_mul(BYTES_PER_PIXEL)
            .ok_or(SurfaceError::ArithmeticOverflow)?;
        let buffer_len = stride
            .checked_mul(height)
            .ok_or(SurfaceError::ArithmeticOverflow)?;

        let mut buffer = Self::allocate_buffer(buffer_len)?;
        for i in (0..buffer.len()).step_by(BYTES_PER_PIXEL) {
            buffer[i] = 0;
            buffer[i + 1] = 0;
            buffer[i + 2] = 0;
            if i + 3 < buffer.len() {
                buffer[i + 3] = 0xFF;
            }
        }

        Ok(Surface {
            width,
            height,
            stride,
            buffer,
        })
    }

    fn allocate_buffer(buffer_len: usize) -> Result<Vec<u8>, SurfaceError> {
        match heap::alloc(buffer_len, 16) {
            Ok(_ptr) => {
                Ok(alloc::vec![0u8; buffer_len])
            }
            Err(_e) => {
                Ok(alloc::vec![0u8; buffer_len])
            }
        }
    }

    pub fn width(&self) -> usize { self.width }
    pub fn height(&self) -> usize { self.height }
    pub fn stride(&self) -> usize { self.stride }
    pub fn buffer(&self) -> &[u8] { &self.buffer }
    pub fn buffer_mut(&mut self) -> &mut [u8] { &mut self.buffer }
    pub fn buffer_len(&self) -> usize { self.buffer.len() }

    fn pixel_offset(&self, x: usize, y: usize) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let row_offset = self.stride.checked_mul(y)?;
        let col_offset = x.checked_mul(BYTES_PER_PIXEL)?;
        row_offset.checked_add(col_offset)
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8) -> Result<(), SurfaceError> {
        let offset = self.pixel_offset(x, y).ok_or(SurfaceError::OutOfBounds)?;
        let end = offset.checked_add(BYTES_PER_PIXEL).ok_or(SurfaceError::ArithmeticOverflow)?;
        if end > self.buffer.len() {
            return Err(SurfaceError::OutOfBounds);
        }
        self.buffer[offset] = r;
        self.buffer[offset + 1] = g;
        self.buffer[offset + 2] = b;
        self.buffer[offset + 3] = 0xFF;
        Ok(())
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> Option<(u8, u8, u8, u8)> {
        let offset = self.pixel_offset(x, y)?;
        if offset + BYTES_PER_PIXEL <= self.buffer.len() {
            Some((self.buffer[offset], self.buffer[offset + 1], self.buffer[offset + 2], self.buffer[offset + 3]))
        } else {
            None
        }
    }

    pub fn clear(&mut self, r: u8, g: u8, b: u8) -> Result<(), SurfaceError> {
        for y in 0..self.height {
            for x in 0..self.width {
                self.set_pixel(x, y, r, g, b)?;
            }
        }
        Ok(())
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, x2: usize, y2: usize, r: u8, g: u8, b: u8) -> Result<(), SurfaceError> {
        if x2 <= x || y2 <= y {
            return Err(SurfaceError::InvalidDimensions);
        }
        let x_start = x.min(self.width);
        let y_start = y.min(self.height);
        let x_end = x2.min(self.width);
        let y_end = y2.min(self.height);
        if x_start >= x_end || y_start >= y_end {
            return Ok(());
        }
        for cy in y_start..y_end {
            for cx in x_start..x_end {
                self.set_pixel(cx, cy, r, g, b)?;
            }
        }
        Ok(())
    }

    pub fn blit_from(&mut self, src: &Surface, dst_x: usize, dst_y: usize) -> Result<(), SurfaceError> {
        for sy in 0..src.height {
            for sx in 0..src.width {
                let dx = dst_x + sx;
                let dy = dst_y + sy;
                if dx < self.width && dy < self.height {
                    if let Some((r, g, b, _)) = src.get_pixel(sx, sy) {
                        self.set_pixel(dx, dy, r, g, b)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn stats(&self) -> SurfaceStats {
        SurfaceStats {
            width: self.width,
            height: self.height,
            stride: self.stride,
            bytes_per_pixel: BYTES_PER_PIXEL,
            buffer_len: self.buffer.len(),
            pixel_count: self.width * self.height,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SurfaceStats {
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub bytes_per_pixel: usize,
    pub buffer_len: usize,
    pub pixel_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_surface() {
        let surface = Surface::new(100, 100).unwrap();
        assert_eq!(surface.width(), 100);
        assert_eq!(surface.height(), 100);
        assert_eq!(surface.stride(), 400);
        assert_eq!(surface.buffer_len(), 40000);
    }

    #[test]
    fn invalid_dimensions() {
        assert!(matches!(Surface::new(0, 100), Err(SurfaceError::InvalidDimensions)));
        assert!(matches!(Surface::new(100, 0), Err(SurfaceError::InvalidDimensions)));
    }

    #[test]
    fn set_get_pixel() {
        let mut surface = Surface::new(10, 10).unwrap();
        surface.set_pixel(5, 5, 255, 128, 64).unwrap();
        let pixel = surface.get_pixel(5, 5).unwrap();
        assert_eq!(pixel, (255, 128, 64, 0xFF));
    }

    #[test]
    fn out_of_bounds_pixel() {
        let mut surface = Surface::new(10, 10).unwrap();
        assert!(matches!(surface.set_pixel(10, 5, 0, 0, 0), Err(SurfaceError::OutOfBounds)));
        assert!(matches!(surface.set_pixel(5, 10, 0, 0, 0), Err(SurfaceError::OutOfBounds)));
    }

    #[test]
    fn clear_surface() {
        let mut surface = Surface::new(5, 5).unwrap();
        surface.clear(10, 20, 30).unwrap();
        for y in 0..5 {
            for x in 0..5 {
                assert_eq!(surface.get_pixel(x, y), Some((10, 20, 30, 0xFF)));
            }
        }
    }

    #[test]
    fn fill_rect_exclusive_edges() {
        let mut surface = Surface::new(10, 10).unwrap();
        surface.fill_rect(2, 2, 5, 5, 255, 0, 0).unwrap();
        assert_eq!(surface.get_pixel(2, 2), Some((255, 0, 0, 0xFF)));
        assert_eq!(surface.get_pixel(4, 4), Some((255, 0, 0, 0xFF)));
        assert_eq!(surface.get_pixel(5, 5), Some((0, 0, 0, 0xFF)));
    }

    #[test]
    fn fill_rect_clipping() {
        let mut surface = Surface::new(10, 10).unwrap();
        surface.fill_rect(5, 5, 20, 20, 0, 255, 0).unwrap();
        assert_eq!(surface.get_pixel(5, 5), Some((0, 255, 0, 0xFF)));
        assert_eq!(surface.get_pixel(9, 9), Some((0, 255, 0, 0xFF)));
    }

    #[test]
    fn fill_rect_completely_offscreen() {
        let mut surface = Surface::new(10, 10).unwrap();
        surface.fill_rect(15, 15, 20, 20, 255, 255, 255).unwrap();
        assert_eq!(surface.get_pixel(0, 0), Some((0, 0, 0, 0xFF)));
    }

    #[test]
    fn fill_rect_invalid_dimensions() {
        let mut surface = Surface::new(10, 10).unwrap();
        assert!(matches!(surface.fill_rect(5, 5, 5, 10, 0, 0, 0), Err(SurfaceError::InvalidDimensions)));
        assert!(matches!(surface.fill_rect(5, 5, 10, 5, 0, 0, 0), Err(SurfaceError::InvalidDimensions)));
    }

    #[test]
    fn blit_from_surface() {
        let mut src = Surface::new(3, 3).unwrap();
        src.fill_rect(0, 0, 3, 3, 100, 150, 200).unwrap();
        let mut dst = Surface::new(10, 10).unwrap();
        dst.blit_from(&src, 2, 3).unwrap();
        assert_eq!(dst.get_pixel(2, 3), Some((100, 150, 200, 0xFF)));
        assert_eq!(dst.get_pixel(4, 5), Some((100, 150, 200, 0xFF)));
    }

    #[test]
    fn surface_stats() {
        let surface = Surface::new(640, 480).unwrap();
        let stats = surface.stats();
        assert_eq!(stats.width, 640);
        assert_eq!(stats.height, 480);
        assert_eq!(stats.bytes_per_pixel, 4);
        assert_eq!(stats.pixel_count, 307200);
    }

    #[test]
    fn alpha_always_opaque() {
        let mut surface = Surface::new(1, 1).unwrap();
        surface.set_pixel(0, 0, 1, 2, 3).unwrap();
        assert_eq!(surface.get_pixel(0, 0).unwrap().3, 0xFF);
    }
}
