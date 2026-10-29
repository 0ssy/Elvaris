//! Basic desktop shell (Phase 18.9).

#![allow(dead_code)]

use alloc::vec::Vec;
use crate::surface::Surface;
use crate::window::WindowManager;

#[derive(Debug)]
pub struct DesktopShell {
    background: Surface,
    panel_height: usize,
    windows: Vec<u32>,
}

impl DesktopShell {
    pub fn new(width: usize, height: usize) -> Result<Self, crate::surface::SurfaceError> {
        let mut background = Surface::new(width, height)?;
        background.clear(20, 40, 80)?;
        Ok(DesktopShell {
            background,
            panel_height: 30,
            windows: Vec::new(),
        })
    }

    pub fn launch_terminal(&mut self, wm: &mut WindowManager, owner: u32) -> Result<u32, crate::window::WindowError> {
        let id = wm.create_window(owner, 100, 50, 400, 300)?;
        self.windows.push(id);
        Ok(id)
    }

    pub fn launch_app(&mut self, wm: &mut WindowManager, owner: u32, name: &str) -> Result<u32, crate::window::WindowError> {
        let id = wm.create_window(owner, 150, 100, 300, 200)?;
        self.windows.push(id);
        let _ = name;
        Ok(id)
    }

    pub fn get_background(&self) -> &Surface {
        &self.background
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn shutdown_authorized(&self, user_id: u32, is_admin: bool) -> Result<(), DesktopError> {
        if !is_admin {
            return Err(DesktopError::Unauthorized);
        }
        if user_id == 0 {
            return Err(DesktopError::Unauthorized);
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DesktopError {
    Unauthorized,
    ShellCrashed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_desktop() {
        let desktop = DesktopShell::new(800, 600).unwrap();
        assert_eq!(desktop.window_count(), 0);
    }

    #[test]
    fn launch_terminal() {
        let mut desktop = DesktopShell::new(800, 600).unwrap();
        let mut wm = WindowManager::new();
        let id = desktop.launch_terminal(&mut wm, 1).unwrap();
        assert_eq!(desktop.window_count(), 1);
        assert!(wm.get_window(id).is_some());
    }

    #[test]
    fn launch_app() {
        let mut desktop = DesktopShell::new(800, 600).unwrap();
        let mut wm = WindowManager::new();
        let _id = desktop.launch_app(&mut wm, 1, "test").unwrap();
        assert_eq!(desktop.window_count(), 1);
    }

    #[test]
    fn authorized_shutdown() {
        let desktop = DesktopShell::new(800, 600).unwrap();
        assert!(desktop.shutdown_authorized(1, true).is_ok());
    }

    #[test]
    fn unauthorized_shutdown() {
        let desktop = DesktopShell::new(800, 600).unwrap();
        assert!(desktop.shutdown_authorized(2, false).is_err());
    }
}
