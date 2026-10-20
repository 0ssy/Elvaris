//! The console of the kernel.
//!
//! The console is how the user programs talk to the person at the keyboard.
//! The output goes to the serial console, and the input comes from an input
//! source.
//!
//! # The input is separated from the driver
//!
//! The console does not know where its input comes from: it asks an
//! [`InputSource`] for the next byte. The serial port is one implementation
//! of the trait, and a keyboard driver would be another one. The same
//! console therefore serves the shell of the milestone and a controlled
//! console that a test drives, without a change to the shell.
//!
//! # The console reads lines
//!
//! A line is read as a whole: the console waits for the newline, handles the
//! backspace, and echoes what it reads. The shell therefore receives a
//! clean line and does not have to know anything about the terminal.

use crate::serial;

/// The longest line the console reads.
pub const MAX_LINE_LENGTH: usize = 1024;

/// The source of the input of the console.
///
/// The trait separates the console from the physical driver.
pub trait InputSource {
    /// Read one byte, if the source has one waiting.
    ///
    /// The function returns `None` when the source has nothing at the
    /// moment. The console waits in that case, until the source gives a
    /// byte or reports that it is exhausted.
    fn read_byte(&mut self) -> Option<u8>;

    /// Whether the source will never give another byte.
    ///
    /// The console stops when the source is exhausted, and reports that no
    /// line was read.
    fn is_exhausted(&self) -> bool {
        false
    }
}

/// The sink of the output of the console.
pub trait Output {
    /// Write the bytes to the console.
    fn write(&mut self, bytes: &[u8]);
}

/// The input of the primary serial console.
#[derive(Debug, Clone, Copy, Default)]
pub struct SerialInput;

impl InputSource for SerialInput {
    fn read_byte(&mut self) -> Option<u8> {
        serial::try_read_byte()
    }
}

/// The output of the primary serial console.
#[derive(Debug, Clone, Copy, Default)]
pub struct SerialOutput;

impl Output for SerialOutput {
    fn write(&mut self, bytes: &[u8]) {
        unsafe { serial::write_bytes(bytes) };
    }
}

/// The console of the kernel.
///
/// The console is the serial console of the machine: the output goes to the
/// serial port, and the input comes from it.
pub struct Console;

impl Console {
    /// Write the bytes to the console.
    ///
    /// A line feed is written as a carriage return and a line feed, so that
    /// the text stays in the column it started in.
    pub fn write(bytes: &[u8]) {
        let mut output = SerialOutput;
        write_to(&mut output, bytes);
    }

    /// Write a string to the console.
    pub fn write_str(text: &str) {
        Self::write(text.as_bytes());
    }

    /// Write a newline to the console.
    pub fn write_newline() {
        unsafe { serial::write_newline() };
    }

    /// Read a line from the console.
    ///
    /// The function returns the length of the line without the newline, or
    /// `None` when the console has no more input. The line is not
    /// terminated: the length tells the caller where it ends.
    pub fn read_line(buffer: &mut [u8]) -> Option<usize> {
        let mut input = SerialInput;
        let mut output = SerialOutput;
        read_line_from(&mut input, &mut output, buffer)
    }
}

/// Write the bytes to a sink, translating the line feeds.
///
/// A line feed that does not follow a carriage return becomes a carriage
/// return and a line feed, so that the text stays in the column it started
/// in. The text that already carries a carriage return is written as it is.
pub fn write_to(output: &mut impl Output, bytes: &[u8]) {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\n' && (index == 0 || bytes[index - 1] != b'\r') {
            output.write(b"\r\n");
        } else {
            output.write(&bytes[index..index + 1]);
        }
        index += 1;
    }
}

/// Read a line from a source into a buffer, echoing what is read.
///
/// The line ends at the newline, at the end of the buffer, or at the end of
/// the input. The newline is not part of the result, and the buffer is not
/// terminated: the length tells the caller where the line ends.
///
/// The carriage return is ignored, so that a terminal that sends both does
/// not produce an empty line. The backspace and the delete character remove
/// the last character of the line, and the console moves the cursor back so
/// that the correction is visible.
///
/// The function waits for the source when it has nothing at the moment, and
/// returns `None` when the source is exhausted.
pub fn read_line_from(
    input: &mut impl InputSource,
    output: &mut impl Output,
    buffer: &mut [u8],
) -> Option<usize> {
    let mut length = 0usize;
    loop {
        let byte = match input.read_byte() {
            Some(byte) => byte,
            None => {
                if input.is_exhausted() {
                    return None;
                }
                // The source has nothing at the moment: the console waits
                // for it.
                core::hint::spin_loop();
                continue;
            }
        };
        match byte {
            b'\r' => continue,
            b'\n' => return Some(length),
            0x08 | 0x7F => {
                if length > 0 {
                    length -= 1;
                    output.write(b"\x08 \x08");
                }
                continue;
            }
            _ => {
                if length < buffer.len() {
                    buffer[length] = byte;
                    length += 1;
                    output.write(&[byte]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A source that hands out the bytes it was given and then reports that
    /// it is exhausted.
    struct FakeSource {
        bytes: Vec<u8>,
        position: usize,
    }

    impl FakeSource {
        fn new(bytes: &[u8]) -> Self {
            Self {
                bytes: bytes.to_vec(),
                position: 0,
            }
        }
    }

    impl InputSource for FakeSource {
        fn read_byte(&mut self) -> Option<u8> {
            let byte = self.bytes.get(self.position).copied();
            if byte.is_some() {
                self.position += 1;
            }
            byte
        }

        fn is_exhausted(&self) -> bool {
            self.position >= self.bytes.len()
        }
    }

    /// A sink that keeps the bytes it was given.
    #[derive(Default)]
    struct FakeOutput {
        bytes: Vec<u8>,
    }

    impl Output for FakeOutput {
        fn write(&mut self, bytes: &[u8]) {
            self.bytes.extend_from_slice(bytes);
        }
    }

    #[test]
    fn a_line_ends_at_the_newline() {
        let mut source = FakeSource::new(b"hello\n");
        let mut output = FakeOutput::default();
        let mut buffer = [0u8; 32];

        let length = read_line_from(&mut source, &mut output, &mut buffer).expect("a line is read");
        assert_eq!(&buffer[..length], b"hello");
        assert_eq!(&output.bytes, b"hello", "the console echoes what it reads");
    }

    #[test]
    fn the_carriage_return_is_ignored() {
        let mut source = FakeSource::new(b"hello\r\n");
        let mut output = FakeOutput::default();
        let mut buffer = [0u8; 32];

        let length = read_line_from(&mut source, &mut output, &mut buffer).expect("a line is read");
        assert_eq!(&buffer[..length], b"hello");
    }

    #[test]
    fn the_backspace_removes_the_last_character() {
        let mut source = FakeSource::new(b"helloX\x08\n");
        let mut output = FakeOutput::default();
        let mut buffer = [0u8; 32];

        let length = read_line_from(&mut source, &mut output, &mut buffer).expect("a line is read");
        assert_eq!(&buffer[..length], b"hello");
    }

    #[test]
    fn a_line_that_does_not_fit_is_cut() {
        let mut source = FakeSource::new(b"abcdef\n");
        let mut output = FakeOutput::default();
        let mut buffer = [0u8; 3];

        let length = read_line_from(&mut source, &mut output, &mut buffer).expect("a line is read");
        assert_eq!(length, 3);
        assert_eq!(&buffer[..length], b"abc");
    }

    #[test]
    fn a_source_without_a_line_reads_nothing() {
        let mut source = FakeSource::new(b"");
        let mut output = FakeOutput::default();
        let mut buffer = [0u8; 32];

        assert_eq!(
            read_line_from(&mut source, &mut output, &mut buffer),
            None,
            "a source that is exhausted ends the input of the console"
        );
    }

    #[test]
    fn a_line_feed_becomes_a_carriage_return_and_a_line_feed() {
        let mut output = FakeOutput::default();
        write_to(&mut output, b"a\nb");
        assert_eq!(&output.bytes, b"a\r\nb");
    }

    #[test]
    fn a_carriage_return_before_a_line_feed_is_kept() {
        let mut output = FakeOutput::default();
        write_to(&mut output, b"a\r\n");
        assert_eq!(&output.bytes, b"a\r\n");
    }
}
