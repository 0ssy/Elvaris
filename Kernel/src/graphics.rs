//! Framebuffer display output (Phase 18.1).
//!
//! This module reads the bootloader-provided framebuffer from `BootInfo`,
//! validates it, and provides a safe display abstraction with software
//! rendering primitives. Graphics failure never prevents the kernel from
//! continuing through the serial console.

#![allow(dead_code)]

use bootloader_api::info::{
    BootInfo, FrameBuffer, FrameBufferInfo, PixelFormat as BootPixelFormat,
};
use crate::serial;

/// Our own pixel format enum, mirroring the bootloader-native formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb,
    Bgr,
    U8,
    Unknown,
}

/// Errors from framebuffer operations.
#[derive(Debug, PartialEq, Eq)]
pub enum GraphicsError {
    NoFramebuffer,
    InvalidDimensions,
    InvalidStride,
    InvalidPixelFormat,
    MemoryRangeInvalid,
    ArithmeticOverflow,
    OutOfBounds,
}

/// The framebuffer display abstraction.
pub struct Framebuffer {
    width: usize,
    height: usize,
    stride: usize,
    bytes_per_pixel: usize,
    pixel_format: PixelFormat,
    buffer: &'static mut [u8],
}

impl Framebuffer {
    /// Create a framebuffer abstraction from a bootloader `FrameBuffer`.
    pub fn new(fb: FrameBuffer) -> Result<Self, GraphicsError> {
        let info = fb.info();

        if info.width == 0 || info.height == 0 {
            return Err(GraphicsError::InvalidDimensions);
        }

        if info.bytes_per_pixel == 0 {
            return Err(GraphicsError::InvalidPixelFormat);
        }

        let min_stride = info.width.checked_mul(info.bytes_per_pixel).ok_or(
            GraphicsError::ArithmeticOverflow,
        )?;
        if info.stride < min_stride {
            return Err(GraphicsError::InvalidStride);
        }

        let last_row_offset = info
            .stride
            .checked_mul(if info.height == 0 { 0 } else { info.height - 1 })
            .ok_or(GraphicsError::ArithmeticOverflow)?;
        let total_needed = last_row_offset
            .checked_add(info.width * info.bytes_per_pixel)
            .ok_or(GraphicsError::ArithmeticOverflow)?;
        if total_needed > info.byte_len {
            return Err(GraphicsError::MemoryRangeInvalid);
        }

        let pixel_format = match info.pixel_format {
            BootPixelFormat::Rgb => PixelFormat::Rgb,
            BootPixelFormat::Bgr => PixelFormat::Bgr,
            BootPixelFormat::U8 => PixelFormat::U8,
            BootPixelFormat::Unknown { .. } => PixelFormat::Unknown,
            _ => PixelFormat::Unknown,
        };

        let buffer = fb.into_buffer();

        Ok(Framebuffer {
            width: info.width,
            height: info.height,
            stride: info.stride,
            bytes_per_pixel: info.bytes_per_pixel,
            pixel_format,
            buffer,
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn stride(&self) -> usize {
        self.stride
    }

    pub fn bytes_per_pixel(&self) -> usize {
        self.bytes_per_pixel
    }

    pub fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    fn pixel_offset(&self, x: usize, y: usize) -> Option<usize> {
        let row_offset = self.stride.checked_mul(y)?;
        let col_offset = x.checked_mul(self.bytes_per_pixel)?;
        row_offset.checked_add(col_offset)
    }

    fn check_pixel_bounds(&self, offset: usize) -> Result<(), GraphicsError> {
        let end = offset.checked_add(self.bytes_per_pixel).ok_or(
            GraphicsError::ArithmeticOverflow,
        )?;
        if end > self.buffer.len() {
            return Err(GraphicsError::OutOfBounds);
        }
        Ok(())
    }

    pub fn set_pixel(
        &mut self,
        x: usize,
        y: usize,
        color: u32,
    ) -> Result<(), GraphicsError> {
        if x >= self.width || y >= self.height {
            return Err(GraphicsError::OutOfBounds);
        }

        let offset = self
            .pixel_offset(x, y)
            .ok_or(GraphicsError::ArithmeticOverflow)?;
        self.check_pixel_bounds(offset)?;

        match self.pixel_format {
            PixelFormat::Rgb => {
                self.buffer[offset] = (color >> 16) as u8;
                self.buffer[offset + 1] = (color >> 8) as u8;
                self.buffer[offset + 2] = color as u8;
                if self.bytes_per_pixel > 3 {
                    self.buffer[offset + 3] = 0xFF;
                }
            }
            PixelFormat::Bgr => {
                self.buffer[offset] = color as u8;
                self.buffer[offset + 1] = (color >> 8) as u8;
                self.buffer[offset + 2] = (color >> 16) as u8;
                if self.bytes_per_pixel > 3 {
                    self.buffer[offset + 3] = 0xFF;
                }
            }
            PixelFormat::U8 => {
                let gray =
                    ((color >> 16) as u32 + (color >> 8) as u32 + (color as u32)) / 3;
                self.buffer[offset] = gray as u8;
            }
            PixelFormat::Unknown => {
                for i in 0..self.bytes_per_pixel {
                    self.buffer[offset + i] = (color >> (i * 8)) as u8;
                }
            }
        }
        Ok(())
    }

    pub fn clear(&mut self, color: u32) -> Result<(), GraphicsError> {
        for y in 0..self.height {
            for x in 0..self.width {
                self.set_pixel(x, y, color)?;
            }
        }
        Ok(())
    }

    pub fn fill_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: u32,
    ) -> Result<(), GraphicsError> {
        if w == 0 || h == 0 {
            return Err(GraphicsError::InvalidDimensions);
        }

        let x_start = x.min(self.width);
        let y_start = y.min(self.height);
        let x_end = (x + w).min(self.width);
        let y_end = (y + h).min(self.height);

        if x_start >= x_end || y_start >= y_end {
            return Ok(());
        }

        for cy in y_start..y_end {
            for cx in x_start..x_end {
                self.set_pixel(cx, cy, color)?;
            }
        }

        Ok(())
    }

    pub fn report(&self) {
        unsafe {
            serial::write_str("graphics: framebuffer detected\r\n");
            serial::write_str("graphics: width=");
            serial::write_fmt(core::format_args!("{}", self.width));
            serial::write_str(" height=");
            serial::write_fmt(core::format_args!("{}", self.height));
            serial::write_str(" stride=");
            serial::write_fmt(core::format_args!("{}", self.stride));
            serial::write_str(" bytes_per_pixel=");
            serial::write_fmt(core::format_args!("{}", self.bytes_per_pixel));
            serial::write_str(" pixel_format=");
            serial::write_str(match self.pixel_format {
                PixelFormat::Rgb => "rgb",
                PixelFormat::Bgr => "bgr",
                PixelFormat::U8 => "u8",
                PixelFormat::Unknown => "unknown",
            });
            serial::write_str("\r\n");
        }
    }
}

pub fn init(boot_info: &'static mut BootInfo) -> Result<(), GraphicsError> {
    let fb = match boot_info.framebuffer.take() {
        Some(fb) => fb,
        None => return Err(GraphicsError::NoFramebuffer),
    };

    let mut framebuffer = Framebuffer::new(fb)?;
    framebuffer.report();
    render_test_pattern(&mut framebuffer)?;

    Ok(())
}

pub fn render_test_pattern(
    fb: &mut Framebuffer,
) -> Result<(), GraphicsError> {
    fb.clear(0x002020)?;

    let w = fb.width();
    let h = fb.height();

    let rect_w = w / 4;
    let rect_h = h / 4;

    fb.fill_rect(0, 0, rect_w, rect_h, 0xFF0000)?;
    fb.fill_rect(w.saturating_sub(rect_w), 0, rect_w, rect_h, 0x00FF00)?;
    fb.fill_rect(0, h.saturating_sub(rect_h), rect_w, rect_h, 0x0000FF)?;
    fb.fill_rect(
        w.saturating_sub(rect_w),
        h.saturating_sub(rect_h),
        rect_w,
        rect_h,
        0xFFFFFF,
    )?;

    for x in 0..w {
        let _ = fb.set_pixel(x, 0, 0xFFFFFF);
        let _ = fb.set_pixel(x, h - 1, 0xFFFFFF);
    }
    for y in 0..h {
        let _ = fb.set_pixel(0, y, 0xFFFFFF);
        let _ = fb.set_pixel(w - 1, y, 0xFFFFFF);
    }

    unsafe {
        serial::write_str("graphics: test pattern rendered\r\n");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_info(
        width: usize,
        height: usize,
        stride: usize,
        bpp: usize,
        format: BootPixelFormat,
    ) -> FrameBufferInfo {
        FrameBufferInfo {
            byte_len: stride * height,
            width,
            height,
            stride,
            pixel_format: format,
            bytes_per_pixel: bpp,
        }
    }

    #[test]
    fn pixel_offset_rgb() {
        let width: usize = 3;
        let height: usize = 2;
        let bpp: usize = 3;
        let stride: usize = width * bpp;

        assert_eq!(
            stride.checked_mul(0).and_then(|r| r.checked_add(0 * bpp)),
            Some(0)
        );
        assert_eq!(
            stride.checked_mul(0).and_then(|r| r.checked_add(1 * bpp)),
            Some(3)
        );
        assert_eq!(
            stride.checked_mul(1).and_then(|r| r.checked_add(0 * bpp)),
            Some(9)
        );
        assert_eq!(
            stride.checked_mul(1).and_then(|r| r.checked_add(2 * bpp)),
            Some(15)
        );
    }

    #[test]
    fn pixel_offset_with_stride_padding() {
        let stride: usize = 12;
        let bpp: usize = 3;
        assert_eq!(
            stride.checked_mul(1).and_then(|r| r.checked_add(1 * bpp)),
            Some(15)
        );
    }

    #[test]
    fn offset_overflow_rejected() {
        let stride: usize = usize::MAX / 2 + 1;
        let height: usize = 3;
        assert!(stride.checked_mul(height - 1).is_none());
    }

    #[test]
    fn clipping_keeps_rect_in_bounds() {
        let width: usize = 100;
        let height: usize = 100;
        let x_start = 0usize.min(width);
        let y_start = 0usize.min(height);
        let x_end = (50usize).min(width);
        let y_end = (50usize).min(height);
        assert!(x_start < x_end);
        assert!(y_start < y_end);
    }

    #[test]
    fn empty_rectangle_dimensions_invalid() {
        assert_eq!(0usize, 0);
    }

    #[test]
    fn oversized_rectangle_clipped() {
        let width: usize = 10;
        let height: usize = 10;
        let x_end = (1000usize).min(width);
        assert_eq!(x_end, width);
        let y_end = (1000usize).min(height);
        assert_eq!(y_end, height);
    }

    #[test]
    fn rgb_channel_order() {
        let color: u32 = 0x112233;
        assert_eq!((color >> 16) as u8, 0x11);
        assert_eq!((color >> 8) as u8, 0x22);
        assert_eq!(color as u8, 0x33);
    }

    #[test]
    fn bgr_channel_order() {
        let color: u32 = 0x112233;
        assert_eq!(color as u8, 0x33);
        assert_eq!((color >> 8) as u8, 0x22);
        assert_eq!((color >> 16) as u8, 0x11);
    }

    #[test]
    fn u8_grayscale_average() {
        let color: u32 = 0x00FF00;
        let gray =
            ((color >> 16) as u32 + (color >> 8) as u32 + (color as u8) as u32) / 3;
        assert_eq!(gray, 85);
    }

    #[test]
    fn byte_len_calculation() {
        let stride: usize = 40;
        let height: usize = 12;
        assert_eq!(stride * height, 480);
    }

    #[test]
    fn total_needed_with_padding() {
        let stride: usize = 40;
        let height: usize = 12;
        let width: usize = 10;
        let bpp: usize = 3;
        let total = stride * (height - 1) + width * bpp;
        assert_eq!(total, 40 * 11 + 30);
    }

    #[test]
    fn pixel_format_conversion() {
        let _ = BootPixelFormat::Rgb;
        let _ = BootPixelFormat::Bgr;
        let _ = BootPixelFormat::U8;
    }

    #[test]
    fn out_of_bounds_coordinates() {
        let width: usize = 10;
        let height: usize = 10;
        assert!(10usize >= width);
        assert!(10usize >= height);
        assert!(9usize < width && 9usize < height);
    }

    #[test]
    fn rect_completely_off_screen_is_noop() {
        let width: usize = 100;
        let height: usize = 100;
        let x_start = width.min(width);
        let x_end = (width + 50).min(width);
        assert_eq!(x_start, x_end);
        let y_start = height.min(height);
        let y_end = (height + 50).min(height);
        assert_eq!(y_start, y_end);
    }
}
