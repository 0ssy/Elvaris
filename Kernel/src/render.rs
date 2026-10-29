//! Software rendering primitives (Phase 18.3).

#![allow(dead_code)]

use crate::surface::{Surface, SurfaceError};

/// Color in RGBA32 format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 0xFF }
    }
    
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }
    
    pub const fn from_u32(c: u32) -> Self {
        Color {
            r: ((c >> 16) & 0xFF) as u8,
            g: ((c >> 8) & 0xFF) as u8,
            b: (c & 0xFF) as u8,
            a: 0xFF,
        }
    }
    
    pub const fn to_u32(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }
    
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0, a: 0xFF };
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255, a: 0xFF };
    pub const RED: Color = Color { r: 255, g: 0, b: 0, a: 0xFF };
    pub const GREEN: Color = Color { r: 0, g: 255, b: 0, a: 0xFF };
    pub const BLUE: Color = Color { r: 0, g: 0, b: 255, a: 0xFF };
    pub const YELLOW: Color = Color { r: 255, g: 255, b: 0, a: 0xFF };
    pub const CYAN: Color = Color { r: 0, g: 255, b: 255, a: 0xFF };
    pub const MAGENTA: Color = Color { r: 255, g: 0, b: 255, a: 0xFF };
}

/// Software renderer operating on surfaces.
pub struct Renderer;

impl Renderer {
    pub fn set_pixel(surface: &mut Surface, x: usize, y: usize, color: Color) -> Result<(), SurfaceError> {
        surface.set_pixel(x, y, color.r, color.g, color.b)
    }

    pub fn clear_surface(surface: &mut Surface, color: Color) -> Result<(), SurfaceError> {
        surface.clear(color.r, color.g, color.b)
    }

    pub fn fill_rect(
        surface: &mut Surface,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: Color,
    ) -> Result<(), SurfaceError> {
        if w == 0 || h == 0 {
            return Err(SurfaceError::InvalidDimensions);
        }
        surface.fill_rect(x, y, x + w, y + h, color.r, color.g, color.b)
    }

    pub fn stroke_rect(
        surface: &mut Surface,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: Color,
    ) -> Result<(), SurfaceError> {
        if w == 0 || h == 0 {
            return Err(SurfaceError::InvalidDimensions);
        }
        if w == 1 && h == 1 {
            return Self::set_pixel(surface, x, y, color);
        }
        Self::draw_hline(surface, x, y, w, color)?;
        if h > 1 {
            Self::draw_hline(surface, x, y + h - 1, w, color)?;
        }
        if w > 1 && h > 2 {
            Self::draw_vline(surface, x, y + 1, h - 2, color)?;
            Self::draw_vline(surface, x + w - 1, y + 1, h - 2, color)?;
        } else if w > 1 && h == 2 {
            Self::draw_vline(surface, x, y, 1, color)?;
            Self::draw_vline(surface, x + w - 1, y, 1, color)?;
        }
        Ok(())
    }

    pub fn draw_hline(
        surface: &mut Surface,
        x: usize,
        y: usize,
        length: usize,
        color: Color,
    ) -> Result<(), SurfaceError> {
        if length == 0 {
            return Ok(());
        }
        let mut drawn = 0;
        let mut px = x;
        while drawn < length && px < surface.width() && y < surface.height() {
            surface.set_pixel(px, y, color.r, color.g, color.b)?;
            px += 1;
            drawn += 1;
        }
        Ok(())
    }

    pub fn draw_vline(
        surface: &mut Surface,
        x: usize,
        y: usize,
        length: usize,
        color: Color,
    ) -> Result<(), SurfaceError> {
        if length == 0 {
            return Ok(());
        }
        let mut drawn = 0;
        let mut py = y;
        while drawn < length && x < surface.width() && py < surface.height() {
            surface.set_pixel(x, py, color.r, color.g, color.b)?;
            py += 1;
            drawn += 1;
        }
        Ok(())
    }

    pub fn draw_line(
        surface: &mut Surface,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: Color,
    ) -> Result<(), SurfaceError> {
        let mut x0 = x0;
        let mut y0 = y0;
        let x1i = x1;
        let y1i = y1;
        
        let dx = (x1i - x0).abs();
        let dy = -(y1i - y0).abs();
        let sx = if x0 < x1i { 1 } else { -1 };
        let sy = if y0 < y1i { 1 } else { -1 };
        let mut err = dx + dy;
        
        loop {
            if x0 >= 0 && y0 >= 0 {
                let ux = x0 as usize;
                let uy = y0 as usize;
                if ux < surface.width() && uy < surface.height() {
                    surface.set_pixel(ux, uy, color.r, color.g, color.b)?;
                }
            }
            if x0 == x1i && y0 == y1i {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
        Ok(())
    }

    pub fn draw_polygon(
        surface: &mut Surface,
        points: &[(i32, i32)],
        color: Color,
    ) -> Result<(), SurfaceError> {
        if points.is_empty() {
            return Ok(());
        }
        for i in 0..points.len() {
            let p0 = points[i];
            let p1 = points[(i + 1) % points.len()];
            Self::draw_line(surface, p0.0, p0.1, p1.0, p1.1, color)?;
        }
        Ok(())
    }

    pub fn fill_triangle(
        surface: &mut Surface,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: Color,
    ) -> Result<(), SurfaceError> {
        let area = ((y1 - y2) as i64 * (x0 - x2) as i64 + (x2 - x1) as i64 * (y0 - y2) as i64).abs();
        if area == 0 {
            return Ok(());
        }
        let mut min_x = x0.min(x1).min(x2);
        let mut max_x = x0.max(x1).max(x2);
        let mut min_y = y0.min(y1).min(y2);
        let mut max_y = y0.max(y1).max(y2);
        if min_x < 0 { min_x = 0; }
        if min_y < 0 { min_y = 0; }
        if max_x >= surface.width() as i32 { max_x = surface.width() as i32 - 1; }
        if max_y >= surface.height() as i32 { max_y = surface.height() as i32 - 1; }
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let area1 = ((y1 - y2) as i64 * (x as i64 - x2 as i64) + (x2 - x1) as i64 * (y as i64 - y2 as i64)).abs();
                let area2 = ((y2 - y0) as i64 * (x as i64 - x2 as i64) + (x0 - x2) as i64 * (y as i64 - y2 as i64)).abs();
                let area3 = ((y0 - y1) as i64 * (x as i64 - x0 as i64) + (x1 - x0) as i64 * (y as i64 - y0 as i64)).abs();
                if area1 + area2 + area3 <= area + 2 {
                    surface.set_pixel(x as usize, y as usize, color.r, color.g, color.b)?;
                }
            }
        }
        Ok(())
    }

    pub fn blit(dst: &mut Surface, src: &Surface, dst_x: usize, dst_y: usize) -> Result<(), SurfaceError> {
        dst.blit_from(src, dst_x, dst_y)
    }

    pub fn stats(surface: &Surface) -> RenderStats {
        RenderStats {
            surface_stats: surface.stats(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderStats {
    pub surface_stats: crate::surface::SurfaceStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::Surface;

    #[test]
    fn test_set_pixel() {
        let mut surface = Surface::new(10, 10).unwrap();
        Renderer::set_pixel(&mut surface, 5, 5, Color::RED).unwrap();
        assert_eq!(surface.get_pixel(5, 5), Some((255, 0, 0, 0xFF)));
    }

    #[test]
    fn test_fill_rect() {
        let mut surface = Surface::new(10, 10).unwrap();
        Renderer::fill_rect(&mut surface, 2, 2, 3, 3, Color::GREEN).unwrap();
        assert_eq!(surface.get_pixel(2, 2), Some((0, 255, 0, 0xFF)));
        assert_eq!(surface.get_pixel(4, 4), Some((0, 255, 0, 0xFF)));
    }

    #[test]
    fn test_stroke_rect() {
        let mut surface = Surface::new(10, 10).unwrap();
        Renderer::stroke_rect(&mut surface, 1, 1, 4, 4, Color::BLUE).unwrap();
        assert_eq!(surface.get_pixel(1, 1), Some((0, 0, 255, 0xFF)));
        assert_eq!(surface.get_pixel(4, 1), Some((0, 0, 255, 0xFF)));
        assert_eq!(surface.get_pixel(1, 4), Some((0, 0, 255, 0xFF)));
        assert_eq!(surface.get_pixel(4, 4), Some((0, 0, 255, 0xFF)));
        assert_eq!(surface.get_pixel(2, 2), Some((0, 0, 0, 0xFF)));
    }

    #[test]
    fn test_draw_hline() {
        let mut surface = Surface::new(10, 10).unwrap();
        Renderer::draw_hline(&mut surface, 2, 3, 4, Color::WHITE).unwrap();
        assert_eq!(surface.get_pixel(2, 3), Some((255, 255, 255, 0xFF)));
        assert_eq!(surface.get_pixel(5, 3), Some((255, 255, 255, 0xFF)));
        assert_eq!(surface.get_pixel(1, 3), Some((0, 0, 0, 0xFF)));
    }

    #[test]
    fn test_draw_vline() {
        let mut surface = Surface::new(10, 10).unwrap();
        Renderer::draw_vline(&mut surface, 3, 2, 4, Color::WHITE).unwrap();
        assert_eq!(surface.get_pixel(3, 2), Some((255, 255, 255, 0xFF)));
        assert_eq!(surface.get_pixel(3, 5), Some((255, 255, 255, 0xFF)));
        assert_eq!(surface.get_pixel(3, 1), Some((0, 0, 0, 0xFF)));
    }

    #[test]
    fn test_draw_line() {
        let mut surface = Surface::new(10, 10).unwrap();
        Renderer::draw_line(&mut surface, 0, 0, 3, 3, Color::RED).unwrap();
        assert_eq!(surface.get_pixel(0, 0), Some((255, 0, 0, 0xFF)));
        assert_eq!(surface.get_pixel(1, 1), Some((255, 0, 0, 0xFF)));
        assert_eq!(surface.get_pixel(3, 3), Some((255, 0, 0, 0xFF)));
    }

    #[test]
    fn test_clipping_out_of_bounds() {
        let mut surface = Surface::new(10, 10).unwrap();
        Renderer::draw_hline(&mut surface, 5, 5, 20, Color::WHITE).unwrap();
        assert_eq!(surface.get_pixel(9, 5), Some((255, 255, 255, 0xFF)));
    }

    #[test]
    fn test_degenerate_shapes() {
        let mut surface = Surface::new(10, 10).unwrap();
        assert!(matches!(Renderer::fill_rect(&mut surface, 0, 0, 0, 5, Color::RED), Err(SurfaceError::InvalidDimensions)));
        assert!(matches!(Renderer::fill_rect(&mut surface, 0, 0, 5, 0, Color::RED), Err(SurfaceError::InvalidDimensions)));
    }

    #[test]
    fn test_draw_polygon() {
        let mut surface = Surface::new(10, 10).unwrap();
        let points = [(0, 0), (5, 0), (2, 5)];
        Renderer::draw_polygon(&mut surface, &points, Color::GREEN).unwrap();
        assert_eq!(surface.get_pixel(0, 0), Some((0, 255, 0, 0xFF)));
        assert_eq!(surface.get_pixel(5, 0), Some((0, 255, 0, 0xFF)));
    }

    #[test]
    fn test_deterministic_rendering() {
        let mut surface = Surface::new(20, 20).unwrap();
        Renderer::clear_surface(&mut surface, Color::BLACK).unwrap();
        Renderer::fill_rect(&mut surface, 0, 0, 10, 10, Color::RED).unwrap();
        Renderer::stroke_rect(&mut surface, 10, 10, 10, 10, Color::WHITE).unwrap();
        Renderer::draw_line(&mut surface, 0, 19, 19, 0, Color::BLUE).unwrap();
        assert_eq!(surface.get_pixel(0, 0), Some((255, 0, 0, 0xFF)));
        assert_eq!(surface.get_pixel(10, 10), Some((255, 255, 255, 0xFF)));
        assert_eq!(surface.get_pixel(0, 19), Some((0, 0, 255, 0xFF)));
    }

    #[test]
    fn test_stats() {
        let surface = Surface::new(640, 480).unwrap();
        let stats = Renderer::stats(&surface);
        assert_eq!(stats.surface_stats.pixel_count, 307200);
    }
}
