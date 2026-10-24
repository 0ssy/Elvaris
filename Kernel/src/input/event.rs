//! Input events abstracted from hardware bytes.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InputEvent {
    Character(u8),
    Backspace,
    Enter,
    ControlC,
    ControlD,
    Unknown,
}

impl InputEvent {
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            b'\r' | b'\n' => InputEvent::Enter,
            0x08 | 0x7F => InputEvent::Backspace,
            0x03 => InputEvent::ControlC,
            0x04 => InputEvent::ControlD,
            b if b >= 0x20 && b < 0x7F => InputEvent::Character(b),
            _ => InputEvent::Unknown,
        }
    }
}
