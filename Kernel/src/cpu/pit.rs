//! The legacy programmable interval timer.
//!
//! The 8253/8254 timer runs at a fixed frequency of 1.193182 MHz. Channel 0
//! is used as the tick source of the kernel: it is programmed in mode 3
//! (square wave) with a divisor that turns it into a periodic interrupt.
//!
//! The frequency is exact and needs no calibration, which makes the timer the
//! simplest reliable tick source in the QEMU machine the kernel runs on. The
//! local APIC timer is used as a second source; see `docs/milestone-6.md`.

use crate::cpu::pic::{read_port, write_port};

/// Data port of channel 0.
pub const PIT_CHANNEL0: u16 = 0x40;

/// Command port of the timer.
pub const PIT_COMMAND: u16 = 0x43;

/// The frequency of the timer in hertz.
pub const PIT_FREQUENCY: u32 = 1_193_182;

/// Control word for channel 0 in mode 3: lobyte/hibyte access, square wave,
/// binary counting.
pub const PIT_MODE3_CHANNEL0: u8 = 0x36;

/// The divisor that turns the timer into the given frequency.
///
/// Returns `None` when the frequency is not reachable: the divisor is
/// sixteen bits wide, and the timer cannot be slower than its own frequency
/// divided by 65536.
///
/// The divisor is rounded to the nearest integer, so the resulting frequency
/// is the closest one the timer can produce.
pub const fn divisor_for_frequency(frequency: u32) -> Option<u16> {
    if frequency == 0 {
        return None;
    }
    let divisor = (PIT_FREQUENCY + frequency / 2) / frequency;
    if divisor == 0 || divisor > 0xFFFF {
        return None;
    }
    Some(divisor as u16)
}

/// The frequency that results from a divisor.
pub const fn frequency_for_divisor(divisor: u16) -> u32 {
    if divisor == 0 {
        return 0;
    }
    PIT_FREQUENCY / divisor as u32
}

/// Program channel 0 to generate interrupts at `frequency` hertz.
///
/// # Safety
/// Port I/O is inherently unsafe, and the timer starts interrupting as soon
/// as it is programmed.
pub unsafe fn set_frequency(frequency: u32) -> Option<u16> {
    let divisor = divisor_for_frequency(frequency)?;
    unsafe {
        write_port(PIT_COMMAND, PIT_MODE3_CHANNEL0);
        write_port(PIT_CHANNEL0, (divisor & 0xFF) as u8);
        write_port(PIT_CHANNEL0, (divisor >> 8) as u8);
    }
    Some(divisor)
}

/// Read the current divisor of channel 0.
pub unsafe fn read_divisor() -> u16 {
    unsafe {
        // Latch the current count before reading it.
        write_port(PIT_COMMAND, 0x40);
        let low = read_port(PIT_CHANNEL0);
        let high = read_port(PIT_CHANNEL0);
        (low as u16) | ((high as u16) << 8)
    }
}
