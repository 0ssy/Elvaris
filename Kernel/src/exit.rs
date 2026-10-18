//! Development-only QEMU termination.
//!
//! This module writes a status code to the QEMU `isa-debugcon` port at I/O
//! base 0x402. When QEMU is started with `-debugcon stdio` (or
//! `-debugcon file:...`), the host can read the code and distinguish:
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

/// Write a 32-bit status code to the debugcon port, low byte first.
///
/// # Safety
/// Port I/O is inherently unsafe. Call this only from the final shutdown path.
pub unsafe fn exit(status: u32) {
    // QEMU's debugcon device accepts one byte at a time on port 0x402.
    // Write the status code as four little-endian bytes.
    write_port(DEBUGCON_PORT, (status & 0xFF) as u8);
    write_port(DEBUGCON_PORT, ((status >> 8) & 0xFF) as u8);
    write_port(DEBUGCON_PORT, ((status >> 16) & 0xFF) as u8);
    write_port(DEBUGCON_PORT, ((status >> 24) & 0xFF) as u8);
}

unsafe fn write_port(port: u16, value: u8) {
    core::arch::asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
    );
}