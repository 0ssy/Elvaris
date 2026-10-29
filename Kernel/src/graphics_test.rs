//! Graphics testing utilities (Phase 18.10).

#![allow(dead_code)]

use crate::surface::Surface;
use crate::render::{Renderer, Color};
use crate::compositor::Compositor;
use crate::window::WindowManager;

pub fn render_test_scene() -> Result<Surface, crate::surface::SurfaceError> {
    let mut surface = Surface::new(800, 600)?;
    Renderer::clear_surface(&mut surface, Color::BLACK)?;
    Renderer::fill_rect(&mut surface, 0, 0, 200, 200, Color::RED)?;
    Renderer::fill_rect(&mut surface, 600, 0, 200, 200, Color::GREEN)?;
    Renderer::fill_rect(&mut surface, 0, 400, 200, 200, Color::BLUE)?;
    Renderer::fill_rect(&mut surface, 600, 400, 200, 200, Color::WHITE)?;
    Renderer::draw_line(&mut surface, 0, 0, 799, 599, Color::YELLOW)?;
    Renderer::stroke_rect(&mut surface, 300, 250, 200, 100, Color::CYAN)?;
    Renderer::draw_hline(&mut surface, 0, 300, 800, Color::MAGENTA)?;
    Renderer::draw_vline(&mut surface, 400, 0, 600, Color::WHITE)?;
    Ok(surface)
}

#[derive(Debug)]
pub struct GraphicsTestSuite;

impl GraphicsTestSuite {
    pub fn test_full_stack() -> Result<(), TestError> {
        let mut surface = Surface::new(640, 480).map_err(|_| TestError::Failed)?;
        surface.clear(0, 0, 0).map_err(|_| TestError::Failed)?;
        Renderer::fill_rect(&mut surface, 10, 10, 100, 100, Color::RED).map_err(|_| TestError::Failed)?;
        let mut compositor = Compositor::new(640, 480).map_err(|_| TestError::Failed)?;
        let mut surf2 = Surface::new(50, 50).map_err(|_| TestError::Failed)?;
        Renderer::fill_rect(&mut surf2, 0, 0, 50, 50, Color::BLUE).map_err(|_| TestError::Failed)?;
        compositor.add_surface(surf2, 200, 200, 1, 1);
        compositor.compose().map_err(|_| TestError::Failed)?;
        let mut wm = WindowManager::new();
        let id = wm.create_window(1, 0, 0, 200, 200).map_err(|_| TestError::Failed)?;
        wm.focus(id).map_err(|_| TestError::Failed)?;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TestError {
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_scene() {
        let surface = render_test_scene().unwrap();
        assert_eq!(surface.width(), 800);
        assert_eq!(surface.height(), 600);
    }

    #[test]
    fn test_full_graphics_stack() {
        assert!(GraphicsTestSuite::test_full_stack().is_ok());
    }
}
