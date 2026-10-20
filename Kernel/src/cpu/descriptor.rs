//! Descriptor encoding for the GDT, the TSS and the IDT.
//!
//! The functions in this module are pure: they turn a description of a
//! descriptor into the bit pattern the processor expects. Nothing here touches
//! hardware, which makes the encoding rules unit-testable on the host target.
//!
//! # Segment descriptor layout (64-bit)
//!
//! ```text
//!  63    56 55 52 51 48 47 40 39 32 31 16 15     0
//! ┌────────┬──┬──┬──┬──┬────────┬────────┬───────┬────────────────┐
//! │ base   │G │D │L │AVL│ type  │ limit  │ base  │     limit      │
//! │ 24..31 │  │  │  │  │ 4 bits │ 16..19 │ 16..23│     0..15      │
//! └────────┴──┴──┴──┴──┴────────┴────────┴───────┴────────────────┘
//! ```
//!
//! # IDT entry layout (64-bit)
//!
//! ```text
//!  63    48 47 40 39 32 31 16 15     0
//! ┌────────┬────────┬────────┬───────┬────────────────┐
//! │ offset │ zero   │ offset │ attr  │     offset     │
//! │ 32..63 │        │ 16..31 │       │     0..15      │
//! └────────┴────────┴────────┴───────┴────────────────┘
//! ```
//!
//! # Task state segment descriptor layout (64-bit)
//!
//! A task state segment descriptor is sixteen bytes wide, so it covers two
//! slots of the global descriptor table. Its low half has the same shape as a
//! system segment descriptor, and its high half holds the upper half of the
//! base address:
//!
//! ```text
//!  95    64 63    56 55 52 51 48 47 40 39 32 31 16 15     0
//! ┌────────┬────────┬────┬────┬────┬────┬────────┬────────┐
//! │  base  │  base  │  0 │limit│type│    │  base  │  base  │
//! │ 63..32 │ 31..24 │    │19..16│   │    │ 23..16 │ 15..0  │
//! └────────┴────────┴────┴────┴────┴────┴────────┴────────┘
//! ```
//!
//! The type field of a 64-bit task state segment is `0b1001`, and the
//! descriptor has to be marked present.

/// Access byte of a long-mode code segment: present, readable, not writable.
pub const CODE_ACCESS: u8 = 0x9A;

/// Access byte of a data segment: present, writable, not executable.
pub const DATA_ACCESS: u8 = 0x92;

/// Flags nibble of a long-mode code segment: long mode, 16-bit default
/// operand size, granularity byte.
pub const CODE_FLAGS: u8 = 0xA;

/// Flags nibble of a data segment: granularity byte, 32-bit default operand
/// size, no long mode.
pub const DATA_FLAGS: u8 = 0xC;

/// Type field of a 64-bit available task state segment descriptor.
pub const TSS_TYPE: u8 = 0b1001;

/// Access byte of a 64-bit available task state segment descriptor.
///
/// The byte holds the type in its low four bits and the present bit in its
/// highest bit. The processor refuses to load a task state segment whose
/// descriptor is not present.
pub const TSS_ACCESS: u8 = 0b1000_1001;

/// Type attribute of a 64-bit interrupt gate.
pub const INTERRUPT_GATE: u8 = 0xE;

/// Type attribute of a 64-bit trap gate.
pub const TRAP_GATE: u8 = 0xF;

/// Encode a code segment descriptor.
pub const fn code_segment(base: u32, limit: u32, dpl: u8) -> u64 {
    let access = CODE_ACCESS | ((dpl & 0x3) << 5);
    segment(base, limit, access, CODE_FLAGS)
}

/// Encode a data segment descriptor.
pub const fn data_segment(base: u32, limit: u32, dpl: u8) -> u64 {
    let access = DATA_ACCESS | ((dpl & 0x3) << 5);
    segment(base, limit, access, DATA_FLAGS)
}

/// Encode a system segment descriptor.
const fn segment(base: u32, limit: u32, access: u8, flags: u8) -> u64 {
    let base = base as u64;
    let limit = limit as u64;
    (limit & 0xFFFF)
        | ((base & 0xFFFF) << 16)
        | (((base >> 16) & 0xFF) << 32)
        | (((limit >> 16) & 0xF) << 48)
        | ((access as u64) << 40)
        | (((flags as u64) & 0xF) << 52)
        | (((base >> 24) & 0xFF) << 56)
}

/// Encode a 64-bit task state segment descriptor.
///
/// A TSS descriptor occupies sixteen bytes: the low eight bytes hold the
/// limit, the base and the type, the high eight bytes hold the upper half of
/// the base address.
pub const fn tss_descriptor(base: u64, limit: u32) -> u128 {
    let limit = limit as u128;
    let base = base as u128;
    (limit & 0xFFFF)
        | ((base & 0xFFFF) << 16)
        | (((base >> 16) & 0xFF) << 32)
        | (((TSS_ACCESS as u128) & 0xFF) << 40)
        | (((limit >> 16) & 0xF) << 48)
        | (((base >> 24) & 0xFF) << 56)
        | ((base >> 32) << 64)
}

/// Encode an interrupt descriptor table entry.
///
/// `ist` selects the interrupt stack table entry (0 = the current stack, 1..7
/// = the dedicated stack of the task state segment). `gate` is
/// [`INTERRUPT_GATE`] or [`TRAP_GATE`].
pub const fn idt_entry(offset: u64, selector: u16, ist: u8, gate: u8) -> u128 {
    idt_entry_with_dpl(offset, selector, ist, gate, 0)
}

/// Encode an interrupt descriptor table entry that user mode may call.
///
/// `dpl` is the requested privilege level of the entry. The processor only
/// lets a program call an entry whose requested privilege level is at least
/// as high as the privilege level of the program, so an entry with a
/// requested privilege level of three is the only kind of entry a user
/// program can reach.
pub const fn idt_entry_with_dpl(
    offset: u64,
    selector: u16,
    ist: u8,
    gate: u8,
    dpl: u8,
) -> u128 {
    // Present, the requested privilege level, 64-bit gate.
    let type_attributes = 0x80 | (((dpl as u128) & 0x3) << 5) | ((gate as u128) & 0xF);
    let offset = offset as u128;
    (offset & 0xFFFF)
        | ((selector as u128) << 16)
        | (((ist as u128) & 0x7) << 32)
        | (type_attributes << 40)
        | (((offset >> 16) & 0xFFFF) << 48)
        | (((offset >> 32) & 0xFFFF_FFFF) << 64)
}

/// Encode the operand of the `lgdt` and `lidt` instructions.
///
/// The operand is a ten-byte structure: a sixteen-bit limit followed by a
/// sixty-four-bit base address.
pub fn table_descriptor(base: u64, limit: u16) -> [u8; 10] {
    let base = base.to_le_bytes();
    let limit = limit.to_le_bytes();
    [
        limit[0], limit[1], base[0], base[1], base[2], base[3], base[4], base[5], base[6],
        base[7],
    ]
}

/// Extract the selector index and the requested privilege level from a
/// segment selector.
///
/// The low three bits of a selector hold the requested privilege level (RPL)
/// and the table indicator (TI); the rest is the index into the table.
pub const fn split_selector(selector: u16) -> (u16, u8, bool) {
    let index = selector & !0x7;
    let rpl = (selector & 0x3) as u8;
    let table = selector & 0x4 != 0;
    (index, rpl, table)
}

/// Combine a selector index and a requested privilege level into a selector.
pub const fn make_selector(index: u16, rpl: u8) -> u16 {
    (index & !0x7) | ((rpl as u16) & 0x3)
}
