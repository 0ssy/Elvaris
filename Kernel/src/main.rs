#![no_std]
#![no_main]

use bootloader_api::entry_point;
use bootloader_api::info::{BootInfo, MemoryRegionKind};

entry_point!(kernel_main);

/// Elvaris kernel entry point.
///
/// The `bootloader` crate jumps here after setting up paging, a kernel stack,
/// and a fresh page table. Interrupts are disabled. The only safe thing to do
/// at this stage is read `BootInfo` and emit a diagnostic message.
fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // Count usable memory regions so we can prove the boot information arrived.
    let usable_count = boot_info
        .memory_regions
        .iter()
        .filter(|r| r.kind == MemoryRegionKind::Usable)
        .count();

    serial_write(b"Elvaris kernel: started on x86-64\r\n");
    serial_write(b"Elvaris kernel: boot information received\r\n");

    let mut buf = [0u8; 64];
    let msg = format_carry(
        &mut buf,
        b"Elvaris kernel: usable memory regions: ",
        usable_count,
    );
    serial_write(msg);
    serial_write(b"\r\n");

    // Halt. No interrupts, no scheduling, no subsystems yet.
    halt();
}

/// Minimal serial output on COM1 (0x3F8).
///
/// This is the primary diagnostic channel for Milestone 1. The bootloader
/// already enables serial logging, so the UART is initialized by the time we
/// run. We write bytes directly to the port.
fn serial_write(bytes: &[u8]) {
    for &b in bytes {
        unsafe {
            core::arch::asm!(
                "out dx, al",
                in("dx") 0x3F8u16,
                in("al") b,
                options(nomem, nostack, preserves_flags)
            );
        }
    }
}

/// Format an unsigned decimal integer into `buf` after `prefix`, returning the
/// full slice written.
fn format_carry<'a>(buf: &'a mut [u8], prefix: &[u8], value: usize) -> &'a [u8] {
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

/// Halt the CPU. With interrupts disabled this is a permanent stop.
fn halt() -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    serial_write(b"Elvaris kernel: panic\r\n");
    halt()
}