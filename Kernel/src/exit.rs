//! Development-only QEMU termination.
//!
//! This module writes a status code to the QEMU `isa-debug-exit` port at I/O
//! base 0x402. When QEMU is started with
//! `-device isa-debug-exit,iobase=0x402,iosize=1`, the device terminates the
//! emulator and the host can read the encoded code and distinguish:
//!
//!   success, kernel panic, unexpected halt, timeout
//!
//! This is a development-only mechanism. It is NOT part of the Elvaris
//! security model and must never be present in a production build.

/// I/O port QEMU reads for the debug-con device.
const DEBUGCON_PORT: u16 = 0x402;

/// Exit code written to the debugcon port to signal a successful boot.
pub const EXIT_SUCCESS: u32 = 0x10;

/// Exit code written to the debugcon port to signal a kernel panic.
pub const EXIT_PANIC: u32 = 0x11;

/// Exit code written to the debugcon port to signal an unexpected halt.
pub const EXIT_UNEXPECTED_HALT: u32 = 0x12;

/// Write the status code to the debugcon port.
///
/// Only a single byte is written. QEMU's `isa-debug-exit` device terminates the
/// emulator on the *first* write to the port and encodes the byte it received
/// in the host exit code. Writing the remaining three bytes of a 32-bit value
/// races with the shutdown that the first byte triggered, so the reported exit
/// code became nondeterministic (the first byte won sometimes, a later zero
/// byte won other times). The status codes are all below 0x100, so one byte
/// carries the whole value.
///
/// # Safety
/// Port I/O is inherently unsafe. Call this only from the final shutdown path.
pub unsafe fn exit(status: u32) {
    // QEMU's debug exit device accepts one byte at a time on port 0x402.
    unsafe {
        write_port(DEBUGCON_PORT, (status & 0xFF) as u8);
    }
}

unsafe fn write_port(port: u16, value: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}