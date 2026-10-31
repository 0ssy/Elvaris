#![no_std]
#![no_main]

extern crate alloc;

mod block;
mod bootinfo;
mod console;
mod cpu;
mod device;
mod exit;
mod fs;
mod heap;
mod input;
mod memory;
mod pmm;
mod pci;
mod device;
mod usb;
mod audio;
mod audio_driver;
mod power;
mod sleep;
mod hotplug;
mod process;
mod security;
mod net;
mod serial;
mod storage;
mod sysinfo;
#[cfg(feature = "bare")]
mod threads;
mod user;
mod virtio;
mod vmm;
mod packages;
mod graphics;
#[cfg(feature = "alloc")]
extern crate alloc;

use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::entry_point;
use bootloader_api::info::{BootInfo, MemoryRegionKind};

use cpu::TIMER_FREQUENCY;
use heap::HEAP_MAPPED_PAGES;
use pmm::phys_alloc::PhysFrameAllocator;
use vmm::mapper::Mapper;

/// Base address of the physical-memory direct map.
///
/// The kernel asks the bootloader for a direct map of physical memory. Without
/// it the kernel cannot dereference its own page tables, so the virtual-memory
/// subsystem and the heap could not work.
pub const PHYSICAL_MEMORY_OFFSET: u64 = 0xFFFF_8000_0000_0000;

/// Number of timer ticks the kernel waits for before it reports success.
const TIMER_TICKS_TARGET: u64 = 5;

/// Upper bound of the spin loop that waits for the timer ticks.
///
/// The bound only matters when the timer does not work at all: the loop gives
/// up instead of hanging the machine.
const TIMER_WAIT_SPINS: u64 = 400_000_000;

/// Configuration handed to the bootloader through the `.bootloader-config`
/// section of the kernel image.
pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::FixedAddress(PHYSICAL_MEMORY_OFFSET));
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

/// The global allocator of the kernel.
///
/// The kernel heap is the only allocator the kernel has, so every allocation
/// the Rust runtime performs goes through it.
#[global_allocator]
static GLOBAL_ALLOCATOR: heap::KernelGlobalAlloc = heap::KernelGlobalAlloc;

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
    // The kernel keeps the numbers of the physical memory: the shell asks
    // the kernel for them long after the boot sequence is over.
    sysinfo::record_frames(
        pmm.total_frames(),
        pmm.allocated_frames(),
        pmm.free_frames(),
    );
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

    // SAFETY: CR3 belongs to the kernel address space and the direct map
    // reported by the bootloader is present in it.
    let mut mapper = match unsafe { Mapper::new(&mut pmm, phys_offset) } {
        Ok(mapper) => mapper,
        Err(err) => panic!("vmm: mapper creation failed: {:?}", err),
    };

    // The registers of the local APIC live in a page outside the usable
    // physical memory, so the bootloader does not map them. The kernel maps
    // the page itself before it touches the local APIC.
    if let Err(err) = mapper.map_page_to(
        cpu::apic::LAPIC_VIRTUAL_BASE,
        cpu::apic::LAPIC_PHYSICAL_BASE,
        cpu::MMIO_PAGE_FLAGS,
    ) {
        panic!("cpu: could not map the local apic registers: {:?}", err);
    }
    cpu::apic::set_base(cpu::apic::LAPIC_VIRTUAL_BASE);
    unsafe {
        serial::write_str("Elvaris kernel: cpu: apic registers mapped at 0x");
        serial::write_fmt(core::format_args!("{:x}", cpu::apic::LAPIC_VIRTUAL_BASE));
        serial::write_newline();
    }

    // Report the state the bootloader left the processor in. The report is
    // taken before the kernel installs its own tables, so it shows the
    // bootloader's global and interrupt descriptor tables.
    cpu::state::report();

    // Kernel heap: map the initial heap pages and hand them to the allocator.
    if let Err(err) = heap::init(&mut mapper) {
        panic!("heap: initialization failed: {:?}", err);
    }
    unsafe {
        serial::write_str("Elvaris kernel: heap: init ok\r\n");
    }

    // The heap must have taken its pages from the PMM: the frame count has to
    // grow by at least the number of heap pages, and by no more than that
    // plus the page-table frames the mapping needed plus the frame the
    // mapper refuses to map (physical address zero).
    let frames_used = mapper.take_allocated_frames();
    if frames_used < HEAP_MAPPED_PAGES as u64 || frames_used > HEAP_MAPPED_PAGES as u64 + 32 {
        panic!("heap: unexpected pmm frame usage: {}", frames_used);
    }
    unsafe {
        serial::write_str("Elvaris kernel: heap: pmm_frames=");
        serial::write_fmt(core::format_args!("{}", frames_used));
        serial::write_str(" ok\r\n");
    }

    // Exercise the heap: allocate, write, verify, free, exhaust.
    if let Err(err) = heap::self_test() {
        panic!("heap: self test failed: {:?}", err);
    }
    unsafe {
        serial::write_str("Elvaris kernel: heap: self_test ok\r\n");
    }

    // Initialize security subsystem
    security::init();
    unsafe {
        serial::write_str("Elvaris kernel: security: initialized\r\n");
    }

    // Initialize package subsystem
    packages::init();
    unsafe {
        serial::write_str("Elvaris kernel: packages: initialized\r\n");
    }

    // Initialize the device manager and discover PCI hardware. Devices are
    // registered as Discovered; drivers bind to them in later phases. Discovery
    // failure does not prevent boot.
    device::init();
    let discovered = device::discover_pci();
    unsafe {
        serial::write_fmt(core::format_args!(
            "Elvaris kernel: device: discovered {} pci devices\r\n",
            discovered
        ));
    }
    device::report();

    // Initialize the input subsystem (Phase 19.2). Physical-style keyboard and
    // mouse devices are registered as DeviceClass::Input and routed through
    // the shared input-event abstraction.
    input::init();
    unsafe {
        serial::write_str("Elvaris kernel: input: initialized\r\n");
    }

    // Discover USB host controllers on the PCI bus (class 0x0C, subclass 0x03)
    // and register them with the USB subsystem. USB enumeration happens as
    // devices attach.
    usb::discover_host_controllers();
    usb::report();

    // Initialize the audio subsystem (Phase 19.4). Audio mixing runs through
    // the audio service; applications do not access audio hardware directly.
    audio::init();
    unsafe {
        serial::write_str("Elvaris kernel: audio: initialized\r\n");
    }

    // Discover physical audio devices (Phase 19.5). A driver is bound when a
    // device is attached; discovery itself never prevents boot.
    let found = audio_driver::discover_audio_devices();
    unsafe {
        serial::write_fmt(core::format_args!(
            "Elvaris kernel: audio: {} device(s) discovered\r\n",
            found
        ));
    }

    // Initialize the power subsystem (Phase 19.6). Power and battery
    // information is exposed read-only; no power state is changed yet.
    power::init();
    unsafe {
        serial::write_str("Elvaris kernel: power: initialized\r\n");
    }

    // Initialize the sleep/resume foundation (Phase 19.7). Suspend requires
    // device ownership and power information to be stable first.
    sleep::init();
    hotplug::init();
    unsafe {
        serial::write_str("Elvaris kernel: sleep: initialized\r\n");
    }

    // Initialize graphics subsystem (Phase 18.1). Failure falls back to
    // serial mode; it never prevents boot.
    match graphics::init(boot_info) {
        Ok(()) => unsafe {
            serial::write_str("Elvaris kernel: graphics: initialized\r\n");
        },
        Err(err) => unsafe {
            serial::write_str("graphics: initialization failed: ");
            serial::write_fmt(core::format_args!("{:?}", err));
            serial::write_str("\r\n");
            serial::write_str("graphics: falling back to serial mode\r\n");
        },
    }

    // Initialize networking
    net::service::init_network();
    unsafe {
        serial::write_str("Elvaris kernel: --- end heap ---\r\n");
    }

    // CPU infrastructure: the global descriptor table, the task state segment,
    // the interrupt descriptor table, the interrupt controllers and the timer.
    // The interrupts stay disabled while this runs.
    let kernel_stack_top = boot_info.kernel_stack_bottom + boot_info.kernel_stack_len;
    let cpu_init = cpu::init(kernel_stack_top);
    unsafe {
        serial::write_str("Elvaris kernel: cpu: kernel_stack_top=0x");
        serial::write_fmt(core::format_args!("{:x}", kernel_stack_top));
        serial::write_str(" timer_hz=");
        serial::write_fmt(core::format_args!("{}", TIMER_FREQUENCY));
        serial::write_newline();
    }

    // Every handler is installed: the interrupts can be enabled now.
    cpu::enable_interrupts();
    unsafe {
        serial::write_str("Elvaris kernel: cpu: interrupts enabled\r\n");
    }

    // Wait for the timer and prove that the interrupt path works.
    let ticks = cpu::wait_for_timer_ticks(TIMER_TICKS_TARGET, TIMER_WAIT_SPINS);
    unsafe {
        serial::write_str("Elvaris kernel: interrupts: timer_ticks=");
        serial::write_fmt(core::format_args!("{}", ticks));
        serial::write_str(" hardware_interrupts=");
        serial::write_fmt(core::format_args!("{}", cpu::timer::hardware_interrupts()));
        serial::write_str(" total=");
        serial::write_fmt(core::format_args!("{}", cpu::timer::total_interrupts()));
        serial::write_newline();
    }
    if ticks == 0 {
        panic!("cpu: no timer interrupt was received");
    }
    unsafe {
        serial::write_str("Elvaris kernel: interrupts: timer ok\r\n");
        serial::write_str("Elvaris kernel: cpu: apic_enabled=");
        serial::write_fmt(core::format_args!("{}", cpu_init.apic_enabled));
        serial::write_str(" pit_divisor=");
        match cpu_init.pit_divisor {
            Some(divisor) => serial::write_fmt(core::format_args!("{}", divisor)),
            None => serial::write_str("none"),
        }
        serial::write_newline();
        serial::write_str("Elvaris kernel: --- end interrupts ---\r\n");
    }

    // The filesystem of the machine. It holds the programs of the userland,
    // and it is the only way a user program reaches a file.
    if let Err(err) = storage::initialize(&mut mapper) {
        panic!("storage: the filesystem could not be created: {:?}", err);
    }
    unsafe {
        serial::write_str("Elvaris kernel: storage: the filesystem is mounted\r\n");
    }

    // User mode: the kernel creates an address space for one user program,
    // loads a fixed image into it, enters ring 3, and takes the program down
    // again when it ends.
    let user_interrupt_stack = match user::allocate_interrupt_stack() {
        Ok(top) => top,
        Err(err) => panic!("user: could not allocate the interrupt stack: {:?}", err),
    };
    // The processor switches to this stack whenever an event arrives from
    // user mode, so the events of the user program never reach the stack the
    // kernel itself runs on.
    cpu::tss::set_privilege_stack(user_interrupt_stack);
    unsafe {
        serial::write_str("Elvaris kernel: user: interrupt stack=0x");
        serial::write_fmt(core::format_args!("{:x}", user_interrupt_stack));
        serial::write_newline();
    }

    user::report();

    let outcome = match user::start(&mut mapper) {
        Ok(outcome) => outcome,
        Err(err) => panic!("user: could not start the user program: {:?}", err),
    };

    user::report_outcome(outcome);

    unsafe {
        serial::write_str("Elvaris kernel: user: syscalls=");
        serial::write_fmt(core::format_args!("{}", user::session::last_syscalls()));
        serial::write_newline();
    }

    // The self test of the user-mode subsystem: two probes that do something
    // the kernel has to refuse. The kernel has to survive both of them.
    if let Err(err) = user::self_test() {
        panic!("user: the self test failed: {:?}", err);
    }

    // The userland: the first program of the machine starts the shell, and
    // the shell runs the programs the person at the console asks for.
    let userland = match user::start_userland() {
        Ok(outcome) => outcome,
        Err(err) => panic!("user: the userland could not start: {:?}", err),
    };
    user::report_outcome(userland);

    unsafe {
        serial::write_str("Elvaris kernel: --- end user ---\r\n");

        serial::write_str("Elvaris kernel: pmm: allocated_frames=");
        serial::write_fmt(core::format_args!("{}", pmm.allocated_frames()));
        serial::write_str(" free_frames=");
        serial::write_fmt(core::format_args!("{}", pmm.free_frames()));
        serial::write_newline();
    }

    // The files of the machine are written back before it stops, so they
    // survive a restart.
    let _ = storage::sync();

    // Signal successful boot to the host via the QEMU debugcon device, then
    // halt. This is development-only; see exit.rs.
    unsafe { exit::exit(exit::EXIT_SUCCESS) };

    // Halt. No scheduling, no subsystems yet.
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
mod surface;
mod render;
mod graphics_input;
mod compositor;
mod window;
mod graphics_session;
mod terminal_emulator;
mod desktop;
mod graphics_test;
