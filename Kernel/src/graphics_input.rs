//! Graphics input events (Phase 18.4).

#![allow(dead_code)]

use alloc::collections::VecDeque;
use alloc::vec::Vec;

/// Input event types for graphics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphicsInputEvent {
    KeyPressed { key: u8, modifiers: u8 },
    KeyReleased { key: u8, modifiers: u8 },
    PointerMoved { x: i32, y: i32 },
    ButtonPressed { button: u8, x: i32, y: i32 },
    ButtonReleased { button: u8, x: i32, y: i32 },
}

/// Input event queue with bounds.
pub struct InputQueue {
    queue: VecDeque<GraphicsInputEvent>,
    max_size: usize,
}

impl InputQueue {
    pub fn new(max_size: usize) -> Self {
        InputQueue {
            queue: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    pub fn push(&mut self, event: GraphicsInputEvent) -> Result<(), InputError> {
        if self.queue.len() >= self.max_size {
            return Err(InputError::QueueFull);
        }
        self.queue.push_back(event);
        Ok(())
    }

    pub fn pop(&mut self) -> Option<GraphicsInputEvent> {
        self.queue.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum InputError {
    QueueFull,
    InvalidEvent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_queue() {
        let queue = InputQueue::new(10);
        assert!(queue.is_empty());
    }

    #[test]
    fn push_pop_events() {
        let mut queue = InputQueue::new(10);
        queue.push(GraphicsInputEvent::KeyPressed { key: 65, modifiers: 0 }).unwrap();
        let event = queue.pop().unwrap();
        assert!(matches!(event, GraphicsInputEvent::KeyPressed { key: 65, .. }));
    }

    #[test]
    fn queue_overflow() {
        let mut queue = InputQueue::new(2);
        queue.push(GraphicsInputEvent::KeyPressed { key: 1, modifiers: 0 }).unwrap();
        queue.push(GraphicsInputEvent::KeyPressed { key: 2, modifiers: 0 }).unwrap();
        assert!(matches!(queue.push(GraphicsInputEvent::KeyPressed { key: 3, modifiers: 0 }), Err(InputError::QueueFull)));
    }

    #[test]
    fn pointer_events() {
        let mut queue = InputQueue::new(10);
        queue.push(GraphicsInputEvent::PointerMoved { x: 100, y: 200 }).unwrap();
        queue.push(GraphicsInputEvent::ButtonPressed { button: 1, x: 100, y: 200 }).unwrap();
        queue.push(GraphicsInputEvent::ButtonReleased { button: 1, x: 100, y: 200 }).unwrap();
    }
}
