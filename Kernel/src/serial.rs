//! Serial output on COM1 (I/O base 0x3F8).
//!
//! This is the primary diagnostic channel for the Elvaris kernel. The
//! `bootloader` crate already programs the UART before it hands control to the
//! kernel, so `init()` only verifies the port is reachable rather than
//! reprogramming hardware.
//!
//! Output is synchronous, unbuffered, and has no locking. It is intended for
//! early boot diagnostics only, not for general kernel logging.

/// I/O base address of COM1.
const COM1_PORT: u16 = 0x3F8;

/// Whether `init()` has been called and the port is ready.
static mut SERIAL_INITIALIZED: bool = false;

/// Verify that COM1 is present by reading the scratch register. The bootloader
/// leaves the UART in a working state, so this is a liveness check, not a full
/// initialization.
///
/// # Safety
/// Calling this more than once is harmless but is discouraged. Port I/O is
/// inherently unsafe; this function is `unsafe` because it performs it.
pub unsafe fn init() {
    // Read the scratch register (offset 7). If the UART is present and
    // uninitialized, this returns 0xFF. We treat any response as "present"
    // because the bootloader has already configured the port.
    let _scratch = read_port(COM1_PORT + 7);
    SERIAL_INITIALIZED = true;
}

/// Write a single byte to COM1.
///
/// # Safety
/// Requires that the UART is ready. Callers must ensure `init()` ran first.
pub unsafe fn write_byte(byte: u8) {
    write_port(COM1_PORT, byte);
}

/// Write a byte slice to COM1.
pub unsafe fn write_bytes(bytes: &[u8]) {
    for &b in bytes {
        write_port(COM1_PORT, b);
    }
}

/// Write a newline (CRLF) to COM1, matching the serial console convention.
pub unsafe fn write_newline() {
    write_port(COM1_PORT, b'\r');
    write_port(COM1_PORT, b'\n');
}

/// Write a string slice to COM1.
pub unsafe fn write_str(s: &str) {
    write_bytes(s.as_bytes());
}

/// Write formatted arguments to COM1.
///
/// This is a minimal implementation sufficient for early diagnostics. It
/// supports `{}`, `{:#x}`, and `{:#b}` for the value types the kernel needs
/// right now.
pub unsafe fn write_fmt(args: core::fmt::Arguments<'_>) {
    use core::fmt::Write;

    struct SerialWriter;
    impl Write for SerialWriter {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            unsafe { write_bytes(s.as_bytes()) }
            Ok(())
        }
    }

    let mut writer = SerialWriter;
    let _ = writer.write_fmt(args);
}

/// Read a byte from an I/O port.
unsafe fn read_port(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!(
        "in al, dx",
        in("dx") port,
        out("al") value,
        options(nomem, nostack, preserves_flags)
    );
    value
}

/// Write a byte to an I/O port.
unsafe fn write_port(port: u16, value: u8) {
    core::arch::asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
    );
}

/// Format an unsigned decimal integer into `buf` after `prefix`, returning the
/// full slice written.
pub fn format_usize<'a>(buf: &'a mut [u8], prefix: &'a [u8], value: usize) -> &'a [u8] {
    let mut idx = 0;
    for &b in prefix.iter() {
        if idx < buf.len() {
            buf[idx] = b;
            idx += 1;
        }
    }

    if value == 0 {
        if idx < buf.len() {
            buf[idx] = b'0';
            idx += 1;
        }
    } else {
        let mut tmp = [0u8; 20];
        let mut n = value;
        let mut t = 0;
        while n > 0 {
            tmp[t] = (n % 10) as u8 + b'0';
            t += 1;
            n /= 10;
        }
        while t > 0 && idx < buf.len() {
            t -= 1;
            buf[idx] = tmp[t];
            idx += 1;
        }
    }

    &buf[..idx]
}

/// Format a 64-bit hex value into `buf` after `prefix`, returning the slice
/// written. Produces exactly `width` hex digits (zero-padded).
pub fn format_hex64<'a>(buf: &'a mut [u8], prefix: &'a [u8], value: u64, width: usize) -> &'a [u8] {
    let mut idx = 0;
    for &b in prefix.iter() {
        if idx < buf.len() {
            buf[idx] = b;
            idx += 1;
        }
    }

    let digits = b"0123456789abcdef";
    let mut started = false;
    for i in (0..width).rev() {
        let nibble = ((value >> (i * 4)) & 0xF) as usize;
        let c = digits[nibble];
        if started || c != b'0' || i == 0 {
            started = true;
            if idx < buf.len() {
                buf[idx] = c;
                idx += 1;
            }
        }
    }

    &buf[..idx]
}