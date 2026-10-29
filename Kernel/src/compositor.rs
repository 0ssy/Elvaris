//! Compositor (Phase 18.5).

#![allow(dead_code)]

use alloc::vec::Vec;
use crate::surface::Surface;

/// Surface entry in compositor.
#[derive(Clone)]
pub struct CompositedSurface {
    pub surface: Surface,
    pub x: usize,
    pub y: usize,
    pub z_order: i32,
    pub visible: bool,
    pub id: u32,
}

/// Software compositor.
pub struct Compositor {
    surfaces: Vec<CompositedSurface>,
    width: usize,
    height: usize,
    composed: Surface,
}

impl Compositor {
    pub fn new(width: usize, height: usize) -> Result<Self, crate::surface::SurfaceError> {
        Ok(Compositor {
            surfaces: Vec::new(),
            width,
            height,
            composed: Surface::new(width, height)?,
        })
    }

    pub fn add_surface(&mut self, surface: Surface, x: usize, y: usize, z_order: i32, id: u32) {
        self.surfaces.push(CompositedSurface { surface, x, y, z_order, visible: true, id });
        self.sort_by_zorder();
    }

    pub fn remove_surface(&mut self, id: u32) {
        self.surfaces.retain(|s| s.id != id);
    }

    pub fn hide_surface(&mut self, id: u32) {
        if let Some(s) = self.surfaces.iter_mut().find(|s| s.id == id) {
            s.visible = false;
        }
    }

    pub fn show_surface(&mut self, id: u32) {
        if let Some(s) = self.surfaces.iter_mut().find(|s| s.id == id) {
            s.visible = true;
        }
    }

    fn sort_by_zorder(&mut self) {
        self.surfaces.sort_by_key(|s| s.z_order);
    }

    pub fn compose(&mut self) -> Result<(), crate::surface::SurfaceError> {
        // Clear composed surface
        self.composed.clear(0, 0, 0)?;
        // Compose from lowest to highest z-order
        for comp in self.surfaces.iter() {
            if comp.visible {
                self.composed.blit_from(&comp.surface, comp.x, comp.y)?;
            }
        }
        Ok(())
    }

    pub fn get_composed(&self) -> &Surface {
        &self.composed
    }

    pub fn get_composed_mut(&mut self) -> &mut Surface {
        &mut self.composed
    }

    pub fn surface_count(&self) -> usize {
        self.surfaces.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::Surface;

    #[test]
    fn create_compositor() {
        let comp = Compositor::new(800, 600).unwrap();
        assert_eq!(comp.surface_count(), 0);
    }

    #[test]
    fn add_and_compose_surface() {
        let mut comp = Compositor::new(100, 100).unwrap();
        let mut surf = Surface::new(10, 10).unwrap();
        surf.fill_rect(0, 0, 10, 10, 255, 0, 0).unwrap();
        comp.add_surface(surf, 0, 0, 0, 1);
        comp.compose().unwrap();
        assert_eq!(comp.get_composed().get_pixel(5, 5), Some((255, 0, 0, 0xFF)));
    }

    #[test]
    fn z_order_composition() {
        let mut comp = Compositor::new(100, 100).unwrap();
        let mut surf1 = Surface::new(20, 20).unwrap();
        surf1.fill_rect(0, 0, 20, 20, 255, 0, 0).unwrap();
        let mut surf2 = Surface::new(20, 20).unwrap();
        surf2.fill_rect(0, 0, 20, 20, 0, 255, 0).unwrap();
        comp.add_surface(surf1, 0, 0, 0, 1);
        comp.add_surface(surf2, 0, 0, 10, 2); // Higher z-order on top
        comp.compose().unwrap();
        assert_eq!(comp.get_composed().get_pixel(5, 5), Some((0, 255, 0, 0xFF)));
    }

    #[test]
    fn hidden_surface() {
        let mut comp = Compositor::new(100, 100).unwrap();
        let mut surf = Surface::new(10, 10).unwrap();
        surf.fill_rect(0, 0, 10, 10, 255, 0, 0).unwrap();
        comp.add_surface(surf, 0, 0, 0, 1);
        comp.hide_surface(1);
        comp.compose().unwrap();
        assert_eq!(comp.get_composed().get_pixel(5, 5), Some((0, 0, 0, 0xFF)));
    }

    #[test]
    fn surface_off_screen() {
        let mut comp = Compositor::new(100, 100).unwrap();
        let mut surf = Surface::new(10, 10).unwrap();
        surf.fill_rect(0, 0, 10, 10, 0, 0, 255).unwrap();
        comp.add_surface(surf, 95, 95, 0, 1);
        comp.compose().unwrap();
        assert_eq!(comp.get_composed().get_pixel(99, 99), Some((0, 0, 255, 0xFF)));
    }

    #[test]
    fn remove_surface() {
        let mut comp = Compositor::new(100, 100).unwrap();
        let surf = Surface::new(10, 10).unwrap();
        comp.add_surface(surf, 0, 0, 0, 1);
        assert_eq!(comp.surface_count(), 1);
        comp.remove_surface(1);
        assert_eq!(comp.surface_count(), 0);
    }
}
