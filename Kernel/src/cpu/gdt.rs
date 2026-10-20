//! The global descriptor table.
//!
//! The kernel installs its own GDT. The bootloader provides one for its own
//! use, but it is a minimal table that only covers what the bootloader needs;
//! the kernel needs its own code and data segments, a pair of user-mode
//! segments for later milestones, and a task state segment descriptor.
//!
//! The table is deliberately flat: the code and data segments cover the whole
//! 4 GiB of the low half and the whole 2^64 range is not relied upon — the
//! segments exist to give the processor a valid privilege level and a valid
//! selector, not to bound the address space.

use core::arch::global_asm;

use crate::cpu::descriptor::{code_segment, data_segment, table_descriptor, tss_descriptor};
use crate::serial;

/// Selector of the kernel code segment (ring 0).
pub const KERNEL_CODE_SELECTOR: u16 = 0x08;

/// Selector of the kernel data segment (ring 0).
pub const KERNEL_DATA_SELECTOR: u16 = 0x10;

/// Selector of the user code segment (ring 3).
pub const USER_CODE_SELECTOR: u16 = 0x18;

/// Selector of the user data segment (ring 3).
pub const USER_DATA_SELECTOR: u16 = 0x20;

/// Selector of the task state segment.
pub const TSS_SELECTOR: u16 = 0x28;

/// Number of eight-byte slots in the table.
///
/// The slot layout is: null, kernel code, kernel data, user code, user data,
/// and the two halves of the task state segment descriptor.
pub const GDT_SLOTS: usize = 7;

global_asm!(
    ".section .text\n",
    ".globl elvaris_gdt_reload_segments\n",
    "elvaris_gdt_reload_segments:\n",
    "    mov ax, 0x10\n",
    "    mov ds, ax\n",
    "    mov es, ax\n",
    "    mov ss, ax\n",
    "    push 0x10\n",
    "    push rsp\n",
    "    add qword ptr [rsp], 8\n",
    "    pushfq\n",
    "    push 0x08\n",
    "    lea rax, [rip + .Lelvaris_gdt_code]\n",
    "    push rax\n",
    "    iretq\n",
    ".Lelvaris_gdt_code:\n",
    "    ret\n",
);

unsafe extern "C" {
    /// Reload the segment registers after the GDT has been installed.
    fn elvaris_gdt_reload_segments();
}

/// The global descriptor table.
#[repr(C, align(8))]
pub struct Gdt {
    entries: [u64; GDT_SLOTS],
}

impl Gdt {
    /// An empty table. Every entry has to be filled before it is loaded.
    pub const fn new() -> Self {
        Self {
            entries: [0u64; GDT_SLOTS],
        }
    }

    /// Replace the eight-byte descriptor in `slot`.
    pub fn set_slot(&mut self, slot: usize, descriptor: u64) {
        self.entries[slot] = descriptor;
    }

    /// The descriptor in `slot`.
    pub fn slot(&self, slot: usize) -> u64 {
        self.entries[slot]
    }

    /// The base address of the table.
    pub fn base_address(&self) -> u64 {
        self as *const Gdt as u64
    }

    /// The limit of the table, in bytes.
    pub fn limit(&self) -> u16 {
        (core::mem::size_of::<Gdt>() - 1) as u16
    }
}

impl Default for Gdt {
    fn default() -> Self {
        Self::new()
    }
}

static mut GDT: Gdt = Gdt::new();

/// Build the table and install it.
///
/// The task state segment descriptor is created from `tss_base` and
/// `tss_limit`, so the task state segment has to be constructed before the
/// table is loaded.
///
/// # Safety
///
/// The caller must ensure that `tss_base`/`tss_limit` describe a valid task
/// state segment and that no interrupt is delivered while the segment
/// registers are being reloaded.
pub unsafe fn install(tss_base: u64, tss_limit: u32) {
    let gdt = unsafe { &mut *core::ptr::addr_of_mut!(GDT) };

    gdt.set_slot(0, 0);
    gdt.set_slot(1, code_segment(0, 0xFFFF_FFFF, 0));
    gdt.set_slot(2, data_segment(0, 0xFFFF_FFFF, 0));
    gdt.set_slot(3, code_segment(0, 0xFFFF_FFFF, 3));
    gdt.set_slot(4, data_segment(0, 0xFFFF_FFFF, 3));

    let tss = tss_descriptor(tss_base, tss_limit);
    gdt.set_slot(5, tss as u64);
    gdt.set_slot(6, (tss >> 64) as u64);

    // Load the table.
    let descriptor = table_descriptor(gdt.base_address(), gdt.limit());
    unsafe {
        core::arch::asm!("lgdt [{}]", in(reg) &descriptor, options(nostack));
    }
    unsafe {
        serial::write_str("Elvaris kernel: cpu: gdt: lgdt done\r\n");
    }

    // The old selectors belong to the bootloader's table: reload them.
    unsafe { elvaris_gdt_reload_segments() };
    unsafe {
        serial::write_str("Elvaris kernel: cpu: gdt: segments reloaded\r\n");
    }

    // Load the task state segment.
    unsafe {
        core::arch::asm!("ltr ax", in("ax") TSS_SELECTOR, options(nomem, nostack, preserves_flags));
    }
    unsafe {
        serial::write_str("Elvaris kernel: cpu: gdt: tss loaded\r\n");
    }
}
/// The base address of the installed table.
pub fn base_address() -> u64 {
    unsafe { (*core::ptr::addr_of!(GDT)).base_address() }
}

/// The limit of the installed table.
pub fn limit() -> u16 {
    unsafe { (*core::ptr::addr_of!(GDT)).limit() }
}
