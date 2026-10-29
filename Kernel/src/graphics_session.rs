//! Graphical session (Phase 18.7).

#![allow(dead_code)]

use crate::compositor::Compositor;
use crate::window::WindowManager;
use crate::graphics_input::InputQueue;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SessionMode {
    Text,
    Graphics,
    Recovery,
}

pub struct GraphicalSession {
    mode: SessionMode,
    compositor: Option<Compositor>,
    window_manager: Option<WindowManager>,
    input_queue: InputQueue,
    running: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SessionError {
    InitializationFailed,
    AlreadyRunning,
}

impl GraphicalSession {
    pub fn new() -> Self {
        GraphicalSession {
            mode: SessionMode::Text,
            compositor: None,
            window_manager: None,
            input_queue: InputQueue::new(256),
            running: false,
        }
    }

    pub fn start_graphics(&mut self, width: usize, height: usize) -> Result<(), SessionError> {
        if self.running {
            return Err(SessionError::AlreadyRunning);
        }
        let compositor = Compositor::new(width, height).map_err(|_| SessionError::InitializationFailed)?;
        self.compositor = Some(compositor);
        self.window_manager = Some(WindowManager::new());
        self.mode = SessionMode::Graphics;
        self.running = true;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.running = false;
        self.mode = SessionMode::Text;
        self.compositor = None;
        self.window_manager = None;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn mode(&self) -> SessionMode {
        self.mode.clone()
    }

    pub fn get_compositor(&self) -> Option<&Compositor> {
        self.compositor.as_ref()
    }

    pub fn get_compositor_mut(&mut self) -> Option<&mut Compositor> {
        self.compositor.as_mut()
    }

    pub fn get_window_manager(&self) -> Option<&WindowManager> {
        self.window_manager.as_ref()
    }

    pub fn get_window_manager_mut(&mut self) -> Option<&mut WindowManager> {
        self.window_manager.as_mut()
    }

    pub fn input_queue(&mut self) -> &mut InputQueue {
        &mut self.input_queue
    }
}

impl Default for GraphicalSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_session() {
        let session = GraphicalSession::new();
        assert!(!session.is_running());
        assert_eq!(session.mode(), SessionMode::Text);
    }

    #[test]
    fn start_graphics_session() {
        let mut session = GraphicalSession::new();
        session.start_graphics(800, 600).unwrap();
        assert!(session.is_running());
        assert_eq!(session.mode(), SessionMode::Graphics);
    }

    #[test]
    fn stop_session() {
        let mut session = GraphicalSession::new();
        session.start_graphics(800, 600).unwrap();
        session.stop();
        assert!(!session.is_running());
        assert_eq!(session.mode(), SessionMode::Text);
    }

    #[test]
    fn session_failure_fallback() {
        let mut session = GraphicalSession::new();
        assert!(session.start_graphics(0, 600).is_err());
        assert_eq!(session.mode(), SessionMode::Text);
    }
}
