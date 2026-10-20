//! The interrupt descriptor table.
//!
//! The IDT tells the processor where the handler of a vector lives. Every
//! entry is sixteen bytes: a thirty-two-bit offset split into three fields, a
//! code segment selector, an interrupt stack table index and the type
//! attributes of the gate.
//!
//! The table has 256 entries. The kernel installs a handler for every CPU
//! exception (vectors 0..=31), for the legacy hardware interrupts
//! (32..=47) and for the timer (0x80). The remaining vectors stay empty: an
//! entry that is not present makes the processor raise a "vector not
//! present" fault, which is handled by the double-fault path.

use crate::cpu::descriptor::{
    idt_entry, idt_entry_with_dpl, table_descriptor, INTERRUPT_GATE,
};
use crate::cpu::vectors::IDT_ENTRY_COUNT;

/// The interrupt descriptor table.
#[repr(C, align(16))]
pub struct Idt {
    entries: [u128; IDT_ENTRY_COUNT],
}

impl Idt {
    /// A table whose entries are all empty.
    pub const fn new() -> Self {
        Self {
            entries: [0; IDT_ENTRY_COUNT],
        }
    }

    /// Install a handler.
    ///
    /// `ist` selects the interrupt stack table entry the processor uses for
    /// this vector; zero means "keep the current stack".
    pub fn set_handler(&mut self, vector: u8, handler: u64, selector: u16, ist: u8) {
        self.set_entry(vector, idt_entry(handler, selector, ist, INTERRUPT_GATE));
    }

    /// Install a handler that user mode may call.
    ///
    /// `dpl` is the requested privilege level of the entry. Only an entry
    /// with a requested privilege level of three can be reached from ring
    /// three.
    pub fn set_handler_with_dpl(
        &mut self,
        vector: u8,
        handler: u64,
        selector: u16,
        ist: u8,
        dpl: u8,
    ) {
        self.set_entry(
            vector,
            idt_entry_with_dpl(handler, selector, ist, INTERRUPT_GATE, dpl),
        );
    }

    /// Install a raw entry.
    pub fn set_entry(&mut self, vector: u8, entry: u128) {
        self.entries[vector as usize] = entry;
    }

    /// The raw entry of `vector`.
    pub fn entry(&self, vector: u8) -> u128 {
        self.entries[vector as usize]
    }

    /// The base address of the table.
    pub fn base_address(&self) -> u64 {
        self as *const Idt as u64
    }

    /// The limit of the table, in bytes.
    pub fn limit(&self) -> u16 {
        (core::mem::size_of::<Idt>() - 1) as u16
    }
}

impl Default for Idt {
    fn default() -> Self {
        Self::new()
    }
}

static mut IDT: Idt = Idt::new();

/// Install a table and load it into the processor.
///
/// # Safety
///
/// The caller must have installed a handler for every event that can occur
/// before the table is loaded, and must not enable interrupts before the
/// handlers are ready.
pub unsafe fn install(table: Idt) {
    let idt = unsafe { &mut *core::ptr::addr_of_mut!(IDT) };
    *idt = table;
    let descriptor = table_descriptor(idt.base_address(), idt.limit());
    unsafe {
        core::arch::asm!("lidt [{}]", in(reg) &descriptor, options(nostack));
    }
}

/// The base address of the installed table.
pub fn base_address() -> u64 {
    unsafe { (*core::ptr::addr_of!(IDT)).base_address() }
}

/// The limit of the installed table.
pub fn limit() -> u16 {
    unsafe { (*core::ptr::addr_of!(IDT)).limit() }
}

/// The raw entry of `vector` in the installed table.
pub fn entry(vector: u8) -> u128 {
    unsafe { (*core::ptr::addr_of!(IDT)).entry(vector) }
}
