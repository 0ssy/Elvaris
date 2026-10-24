//! Fixed-capacity input event queue.

use super::event::InputEvent;

pub const QUEUE_CAPACITY: usize = 256;

pub struct InputQueue {
    buf: [Option<InputEvent>; QUEUE_CAPACITY],
    head: usize,
    tail: usize,
    count: usize,
    overflows: usize,
}

impl InputQueue {
    pub const fn new() -> Self {
        Self {
            buf: [None; QUEUE_CAPACITY],
            head: 0,
            tail: 0,
            count: 0,
            overflows: 0,
        }
    }

    pub fn push(&mut self, event: InputEvent) -> bool {
        if self.count == QUEUE_CAPACITY {
            self.overflows += 1;
            return false;
        }
        self.buf[self.tail] = Some(event);
        self.tail = (self.tail + 1) % QUEUE_CAPACITY;
        self.count += 1;
        true
    }

    pub fn pop(&mut self) -> Option<InputEvent> {
        if self.count == 0 {
            return None;
        }
        let ev = self.buf[self.head].take();
        self.head = (self.head + 1) % QUEUE_CAPACITY;
        self.count -= 1;
        ev
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn is_full(&self) -> bool {
        self.count == QUEUE_CAPACITY
    }

    pub fn overflows(&self) -> usize {
        self.overflows
    }
}
