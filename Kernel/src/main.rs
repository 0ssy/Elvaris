#![no_std]
#![no_main]

mod serial;
mod pmm;
mod bootinfo;
mod memory;
mod exit;
mod vmm;
mod heap;

use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::entry_point;
use bootloader_api::info::{BootInfo, MemoryRegionKind};

use heap::HEAP_MAPPED_PAGES;
use pmm::phys_alloc::PhysFrameAllocator;
use vmm::mapper::Mapper;

/// Base address of the physical-memory direct map.
///
/// The kernel asks the bootloader for a direct map of physical memory. Without
/// it the kernel cannot dereference its own page tables, so the virtual-memory
/// subsystem and the heap could not work.
pub const PHYSICAL_MEMORY_OFFSET: u64 = 0xFFFF_8000_0000_0000;

/// Configuration handed to the bootloader through the `.bootloader-config`
/// section of the kernel image.
pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::FixedAddress(PHYSICAL_MEMORY_OFFSET));
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

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
    let mut pmm = PhysFrameAllocator::new();
    if !pmm.init(regions) {
        panic!("pmm: initialization failed");
    }
    unsafe {
        serial::write_str("Elvaris kernel: pmm: usable_regions=");
        serial::write_fmt(core::format_args!("{}", pmm.usable_region_count()));
        serial::write_str(" total_frames=");
        serial::write_fmt(core::format_args!("{}", pmm.total_frames()));
        serial::write_str(" allocated_frames=");
        serial::write_fmt(core::format_args!("{}", pmm.allocated_frames()));
        serial::write_newline();
    }

    // The physical-memory direct map is what makes the page tables reachable.
    let phys_offset = boot_info.physical_memory_offset.as_ref().copied();
    if phys_offset.is_none() {
        panic!("vmm: the bootloader did not provide a physical memory mapping");
    }
    let frames_before = pmm.allocated_frames();

    // Kernel heap: map the initial heap pages and hand them to the allocator.
    {
        // SAFETY: CR3 belongs to the kernel address space and the direct map
        // reported by the bootloader is present in it.
        let mut mapper = match unsafe { Mapper::new(&mut pmm, phys_offset) } {
            Ok(mapper) => mapper,
            Err(err) => panic!("vmm: mapper creation failed: {:?}", err),
        };
        if let Err(err) = heap::init(&mut mapper) {
            panic!("heap: initialization failed: {:?}", err);
        }
    }
    unsafe {
        serial::write_str("Elvaris kernel: heap: init ok\r\n");
    }

    // The heap must have taken its pages from the PMM: the frame count has to
    // grow by at least the number of heap pages, and by no more than that
    // plus the page-table frames the mapping needed plus the frame the
    // mapper refuses to map (physical address zero).
    let frames_used = pmm.allocated_frames() - frames_before;
    if frames_used < HEAP_MAPPED_PAGES as u64 || frames_used > HEAP_MAPPED_PAGES as u64 + 32 {
        panic!("heap: unexpected pmm frame usage: {}", frames_used);
    }
    unsafe {
        serial::write_str("Elvaris kernel: heap: pmm_frames=");
        serial::write_fmt(core::format_args!("{}", frames_used));
        serial::write_str(" ok\r\n");
        serial::write_str("Elvaris kernel: pmm: allocated_frames=");
        serial::write_fmt(core::format_args!("{}", pmm.allocated_frames()));
        serial::write_str(" free_frames=");
        serial::write_fmt(core::format_args!("{}", pmm.free_frames()));
        serial::write_newline();
    }

    // Exercise the heap: allocate, write, verify, free, exhaust.
    if let Err(err) = heap::self_test() {
        panic!("heap: self test failed: {:?}", err);
    }
    unsafe {
        serial::write_str("Elvaris kernel: heap: self_test ok\r\n");
    }

    // Report the heap configuration and statistics.
    heap::report();
    unsafe {
        serial::write_str("Elvaris kernel: --- end heap ---\r\n");
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
