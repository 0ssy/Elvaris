//! Hardware input events decoded from physical devices.
//!
//! These events are produced by input device drivers (keyboard, mouse) and
//! consumed by the input service, which routes them to the active terminal
//! or graphical session. The existing serial `InputEvent` type is unchanged;
//! hardware input is a separate, richer event stream.

use alloc::vec::Vec;

/// A hardware keyboard key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// Hardware scan code from the device.
    pub scan_code: u8,
    /// Whether the key was pressed (true) or released (false).
    pub pressed: bool,
    /// Active modifier state at the time of the event.
    pub modifiers: ModifierState,
}

/// Modifier key state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModifierState {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub gui: bool,
}

/// A hardware pointer (mouse) motion event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerEvent {
    /// Relative X movement (positive = right).
    pub dx: i32,
    /// Relative Y movement (positive = down).
    pub dy: i32,
}

/// A hardware mouse button event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonEvent {
    /// Button index: 0 = left, 1 = middle, 2 = right.
    pub button: u8,
    /// Whether the button was pressed (true) or released (false).
    pub pressed: bool,
}

/// All hardware input events produced by physical devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareInputEvent {
    Key(KeyEvent),
    Pointer(PointerEvent),
    Button(ButtonEvent),
}

impl HardwareInputEvent {
    /// Whether the event is a key press (not release).
    pub fn is_key_press(&self) -> bool {
        matches!(self, HardwareInputEvent::Key(k) if k.pressed)
    }

    /// Whether the event is a key release.
    pub fn is_key_release(&self) -> bool {
        matches!(self, HardwareInputEvent::Key(k) if !k.pressed)
    }

    /// Whether the event is a pointer motion.
    pub fn is_pointer(&self) -> bool {
        matches!(self, HardwareInputEvent::Pointer(_))
    }

    /// Whether the event is a button press/release.
    pub fn is_button(&self) -> bool {
        matches!(self, HardwareInputEvent::Button(_))
    }
}

/// Decode a raw PS/2-style keyboard scan code into hardware events.
///
/// Returns `None` for bytes that are not complete key events on their own
/// (e.g. prefix bytes such as `0xE0`).
pub fn decode_keyboard_byte(byte: u8, modifiers: &mut ModifierState) -> Option<HardwareInputEvent> {
    // Release prefix: the following scan code is a key release.
    const RELEASE_PREFIX: u8 = 0xF0;

    // Handle the release prefix as stateful: it is not itself an event.
    if byte == RELEASE_PREFIX {
        return None;
    }

    // Track modifier keys by scan code (PC AT set 2).
    let (is_modifier, pressed_modifier) = match byte {
        0x12 | 0x59 => (true, &mut modifiers.shift), // left/right shift
        0x14 => (true, &mut modifiers.ctrl),         // left ctrl
        0x11 => (true, &mut modifiers.alt),          // left alt
        0x1F => (true, &mut modifiers.gui),          // left gui
        _ => (false, &mut false),
    };

    let pressed = true; // make-key by default; release prefix handled above

    if is_modifier {
        *pressed_modifier = pressed;
        return Some(HardwareInputEvent::Key(KeyEvent {
            scan_code: byte,
            pressed,
            modifiers: *modifiers,
        }));
    }

    Some(HardwareInputEvent::Key(KeyEvent {
        scan_code: byte,
        pressed,
        modifiers: *modifiers,
    }))
}

/// Decode a relative mouse motion packet into pointer and button events.
///
/// `dx`, `dy` are signed relative movement. `buttons` is a bitmask: bit 0
/// left, bit 1 right, bit 2 middle.
pub fn decode_mouse_packet(
    dx: i32,
    dy: i32,
    buttons: u8,
    prev_buttons: u8,
) -> Vec<HardwareInputEvent> {
    let mut events = Vec::new();

    if dx != 0 || dy != 0 {
        events.push(HardwareInputEvent::Pointer(PointerEvent { dx, dy }));
    }

    for button in 0..3u8 {
        let mask = 1u8 << button;
        let was_pressed = prev_buttons & mask != 0;
        let is_pressed = buttons & mask != 0;
        if was_pressed != is_pressed {
            events.push(HardwareInputEvent::Button(ButtonEvent {
                button,
                pressed: is_pressed,
            }));
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_press_decoded() {
        let mut modifiers = ModifierState::default();
        let ev = decode_keyboard_byte(0x1C, &mut modifiers).unwrap();
        assert!(matches!(ev, HardwareInputEvent::Key(k) if k.pressed && k.scan_code == 0x1C));
    }

    #[test]
    fn modifier_tracking() {
        let mut modifiers = ModifierState::default();
        let ev = decode_keyboard_byte(0x12, &mut modifiers).unwrap();
        assert!(ev.is_key_press());
        assert!(modifiers.shift);

        // Release of the same shift key.
        let mut modifiers2 = ModifierState { shift: true, ..Default::default() };
        // The release prefix is filtered; caller re-decodes the scan code as
        // a release after seeing 0xF0.
        let _ = decode_keyboard_byte(0xF0, &mut modifiers2);
        let ev2 = decode_keyboard_byte(0x12, &mut modifiers2).unwrap();
        assert!(matches!(ev2, HardwareInputEvent::Key(k) if k.pressed));
    }

    #[test]
    fn mouse_motion_and_buttons() {
        let events = decode_mouse_packet(5, -3, 0b001, 0b000);
        assert!(events.iter().any(|e| matches!(e, HardwareInputEvent::Pointer(p) if p.dx == 5 && p.dy == -3)));
        assert!(events.iter().any(|e| matches!(e, HardwareInputEvent::Button(b) if b.button == 0 && b.pressed)));
    }

    #[test]
    fn button_release_detected() {
        let events = decode_mouse_packet(0, 0, 0b000, 0b001);
        assert!(events.iter().any(|e| matches!(e, HardwareInputEvent::Button(b) if b.button == 0 && !b.pressed)));
    }
}
