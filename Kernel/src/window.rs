//! Window management (Phase 18.6).

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use crate::surface::Surface;

#[derive(Clone)]
pub struct Window {
    pub id: u32,
    pub owner: u32,
    pub x: i32,
    pub y: i32,
    pub width: usize,
    pub height: usize,
    pub visible: bool,
    pub focused: bool,
    pub z_order: i32,
    pub surface: Surface,
}

pub struct WindowManager {
    windows: BTreeMap<u32, Window>,
    next_id: u32,
    focused_window: Option<u32>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum WindowError {
    InvalidWindow,
    Unauthorized,
    InvalidOperation,
}

impl WindowManager {
    pub fn new() -> Self {
        WindowManager {
            windows: BTreeMap::new(),
            next_id: 1,
            focused_window: None,
        }
    }

    pub fn create_window(&mut self, owner: u32, x: i32, y: i32, width: usize, height: usize) -> Result<u32, WindowError> {
        if width == 0 || height == 0 {
            return Err(WindowError::InvalidOperation);
        }
        let surface = Surface::new(width, height).map_err(|_| WindowError::InvalidOperation)?;
        let id = self.next_id;
        self.next_id += 1;
        let window = Window {
            id,
            owner,
            x,
            y,
            width,
            height,
            visible: true,
            focused: false,
            z_order: 0,
            surface,
        };
        self.windows.insert(id, window);
        if self.focused_window.is_none() {
            let _ = self.focus(id);
        }
        Ok(id)
    }

    pub fn destroy_window(&mut self, window_id: u32, requester: u32) -> Result<(), WindowError> {
        let window = self.windows.get(&window_id).ok_or(WindowError::InvalidWindow)?;
        if window.owner != requester {
            return Err(WindowError::Unauthorized);
        }
        self.windows.remove(&window_id);
        if self.focused_window == Some(window_id) {
            self.focused_window = self.windows.keys().next().copied();
            if let Some(fid) = self.focused_window {
                if let Some(w) = self.windows.get_mut(&fid) {
                    w.focused = true;
                }
            }
        }
        Ok(())
    }

    pub fn focus(&mut self, window_id: u32) -> Result<(), WindowError> {
        if !self.windows.contains_key(&window_id) {
            return Err(WindowError::InvalidWindow);
        }
        if let Some(old_id) = self.focused_window {
            if let Some(w) = self.windows.get_mut(&old_id) {
                w.focused = false;
            }
        }
        if let Some(w) = self.windows.get_mut(&window_id) {
            w.focused = true;
        }
        self.focused_window = Some(window_id);
        Ok(())
    }

    pub fn get_focused(&self) -> Option<u32> {
        self.focused_window
    }

    pub fn move_window(&mut self, window_id: u32, requester: u32, x: i32, y: i32) -> Result<(), WindowError> {
        let window = self.windows.get_mut(&window_id).ok_or(WindowError::InvalidWindow)?;
        if window.owner != requester {
            return Err(WindowError::Unauthorized);
        }
        window.x = x;
        window.y = y;
        Ok(())
    }

    pub fn show_window(&mut self, window_id: u32, requester: u32) -> Result<(), WindowError> {
        let window = self.windows.get_mut(&window_id).ok_or(WindowError::InvalidWindow)?;
        if window.owner != requester {
            return Err(WindowError::Unauthorized);
        }
        window.visible = true;
        Ok(())
    }

    pub fn hide_window(&mut self, window_id: u32, requester: u32) -> Result<(), WindowError> {
        let window = self.windows.get_mut(&window_id).ok_or(WindowError::InvalidWindow)?;
        if window.owner != requester {
            return Err(WindowError::Unauthorized);
        }
        window.visible = false;
        Ok(())
    }

    pub fn get_window(&self, window_id: u32) -> Option<&Window> {
        self.windows.get(&window_id)
    }

    pub fn get_window_mut(&mut self, window_id: u32) -> Option<&mut Window> {
        self.windows.get_mut(&window_id)
    }

    pub fn windows(&self) -> Vec<&Window> {
        self.windows.values().collect()
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_window() {
        let mut wm = WindowManager::new();
        let _id = wm.create_window(1, 10, 10, 100, 100).unwrap();
        assert_eq!(wm.window_count(), 1);
    }

    #[test]
    fn window_ownership() {
        let mut wm = WindowManager::new();
        let id = wm.create_window(1, 10, 10, 100, 100).unwrap();
        assert!(wm.destroy_window(id, 2).is_err());
        assert!(wm.destroy_window(id, 1).is_ok());
    }

    #[test]
    fn focus_management() {
        let mut wm = WindowManager::new();
        let id1 = wm.create_window(1, 0, 0, 100, 100).unwrap();
        let id2 = wm.create_window(1, 10, 10, 100, 100).unwrap();
        wm.focus(id1).unwrap();
        assert_eq!(wm.get_focused(), Some(id1));
        wm.focus(id2).unwrap();
        assert_eq!(wm.get_focused(), Some(id2));
    }

    #[test]
    fn move_window() {
        let mut wm = WindowManager::new();
        let id = wm.create_window(1, 0, 0, 100, 100).unwrap();
        wm.move_window(id, 1, 50, 50).unwrap();
        assert_eq!(wm.get_window(id).unwrap().x, 50);
    }

    #[test]
    fn visibility() {
        let mut wm = WindowManager::new();
        let id = wm.create_window(1, 0, 0, 100, 100).unwrap();
        wm.hide_window(id, 1).unwrap();
        assert!(!wm.get_window(id).unwrap().visible);
        wm.show_window(id, 1).unwrap();
        assert!(wm.get_window(id).unwrap().visible);
    }

    #[test]
    fn two_windows() {
        let mut wm = WindowManager::new();
        let _id1 = wm.create_window(1, 0, 0, 200, 200).unwrap();
        let _id2 = wm.create_window(2, 50, 50, 200, 200).unwrap();
        assert_eq!(wm.window_count(), 2);
    }
}
