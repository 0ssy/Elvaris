pub mod event;
pub mod queue;
pub mod hardware;
pub mod device;

pub use event::InputEvent;
pub use queue::{InputQueue, QUEUE_CAPACITY};

#[cfg(test)]
mod tests {
    use super::event::InputEvent;
    use super::queue::InputQueue;

    #[test]
    fn event_from_byte() {
        assert_eq!(InputEvent::from_byte(b'a'), InputEvent::Character(b'a'));
        assert_eq!(InputEvent::from_byte(b'\r'), InputEvent::Enter);
        assert_eq!(InputEvent::from_byte(b'\n'), InputEvent::Enter);
        assert_eq!(InputEvent::from_byte(0x08), InputEvent::Backspace);
        assert_eq!(InputEvent::from_byte(0x7F), InputEvent::Backspace);
        assert_eq!(InputEvent::from_byte(0x03), InputEvent::ControlC);
        assert_eq!(InputEvent::from_byte(0x04), InputEvent::ControlD);
    }

    #[test]
    fn queue_push_pop() {
        let mut q = InputQueue::new();
        assert!(q.is_empty());
        assert!(q.push(InputEvent::Character(b'x')));
        assert_eq!(q.pop(), Some(InputEvent::Character(b'x')));
        assert!(q.is_empty());
    }

    #[test]
    fn queue_overflow() {
        let mut q = InputQueue::new();
        for _ in 0..256 {
            assert!(q.push(InputEvent::Character(b'a')));
        }
        assert!(q.is_full());
        assert!(!q.push(InputEvent::Character(b'b')));
        assert_eq!(q.overflows(), 1);
    }
}
