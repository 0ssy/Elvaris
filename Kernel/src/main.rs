#![no_std]
#![no_main]

mod serial;
mod pmm;
mod bootinfo;
mod memory;
mod exit;

use bootloader_api::entry_point;
use bootloader_api::info::{BootInfo, MemoryRegionKind};

entry_point!(kernel_main);

/// Elvaris kernel entry point.
///
/// The `bootloader` crate jumps here after setting up paging, a kernel stack,
/// and a fresh page table. Interrupts are disabled. The only safe thing to do
/// at this stage is read `BootInfo` and emit a diagnostic message.
fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // SAFETY: this is the first code the kernel runs after the bootloader
    // hands off control. COM1 is already programmed by the bootloader.
    unsafe { serial::init() };

    unsafe {
        serial::write_str("Elvaris kernel: started on x86-64\r\n");
        serial::write_str("Elvaris kernel: boot information received\r\n");

        // Count usable memory regions so we can prove the boot information
        // arrived.
        let usable_count = boot_info
            .memory_regions
            .iter()
            .filter(|r| r.kind == MemoryRegionKind::Usable)
            .count();

        let mut buf = [0u8; 64];
        let msg = serial::format_usize(
            &mut buf,
            b"Elvaris kernel: usable memory regions: ",
            usable_count,
        );
        serial::write_bytes(msg);
        serial::write_newline();
    }

    // Inspect and report the full BootInfo inventory.
    bootinfo::report(boot_info);

    // Validate and report the memory map.
    let regions: &[bootloader_api::info::MemoryRegion] = boot_info.memory_regions.as_ref();
    // SAFETY: the bootloader guarantees the memory_regions slice is valid for
    // the lifetime of BootInfo, which is 'static mut at entry.
    let summary = unsafe { memory::validate_and_summarize(regions) };
    memory::report_summary(&summary);

    // Physical memory management initialization.
    let mut pmm = pmm::phys_alloc::PhysFrameAllocator::new();
    let _ = pmm.init(regions);
    unsafe {
        serial::write_str("Elvaris kernel: pmm: usable_regions=");
        serial::write_fmt(core::format_args!("{}", pmm.usable_region_count()));
        serial::write_str(" total_frames=");
        serial::write_fmt(core::format_args!("{}", pmm.total_frames()));
        serial::write_newline();
    }

    // Signal successful boot to the host via the QEMU debugcon device, then
    // halt. This is development-only; see exit.rs.
    unsafe { exit::exit(exit::EXIT_SUCCESS) };

    // Halt. No interrupts, no scheduling, no subsystems yet.
    halt()
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
fn panic(info: &core::panic::PanicInfo) -> ! {
    unsafe {
        serial::write_str("Elvaris kernel: PANIC\r\n");

        // Print the panic message if one was provided.
        let msg = info.message();
        serial::write_str("Elvaris kernel: message: ");
        serial::write_fmt(core::format_args!("{}", msg));
        serial::write_newline();

        // Print the source location where available.
        if let Some(loc) = info.location() {
            serial::write_str("Elvaris kernel: location: ");
            serial::write_fmt(core::format_args!(
                "{}:{}:{}",
                loc.file(),
                loc.line(),
                loc.column()
            ));
            serial::write_newline();
        } else {
            serial::write_str("Elvaris kernel: location: unknown\r\n");
        }

        // Signal the panic exit code to the host before halting.
        exit::exit(exit::EXIT_PANIC);
    }

    halt()
}
