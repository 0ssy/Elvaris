//! CPU exception vectors, error codes and the captured register state.
//!
//! This module contains the pure part of the interrupt infrastructure: the
//! vector numbering, the metadata that describes every CPU exception (its
//! name, whether it carries an error code, whether it is reserved), and the
//! layout of the register state the CPU pushes when an event is delivered.
//!
//! Keeping the metadata here — instead of in the handler — makes it
//! unit-testable on the host target and gives the diagnostics a single source
//! of truth.

/// Number of vectors reserved for CPU exceptions (0..=31).
pub const EXCEPTION_VECTOR_COUNT: u8 = 32;

/// First vector available for hardware interrupts.
///
/// The legacy interrupt controllers are remapped so that the hardware
/// interrupts start here, immediately after the CPU exception vectors. This
/// keeps hardware interrupts from colliding with exception vectors.
pub const HARDWARE_VECTOR_BASE: u8 = 32;

/// Last vector available for hardware interrupts (IRQ15).
pub const HARDWARE_VECTOR_LAST: u8 = 47;

/// Vector used by the kernel timer.
///
/// The vector is deliberately outside the ranges used by the CPU exceptions
/// and by the legacy interrupt controllers, so a stray interrupt cannot be
/// mistaken for a timer tick.
pub const TIMER_VECTOR: u8 = 0x40;

/// Vector used by the system-call instruction of the user mode.
///
/// The entry of this vector is the only one the processor accepts from user
/// mode: its requested privilege level is three, while every other entry
/// belongs to ring zero. A user program therefore cannot raise any other
/// interrupt, and the kernel stays in control of the entry points.
pub const SYSCALL_VECTOR: u8 = 0x80;

/// Number of descriptors in the interrupt descriptor table.
pub const IDT_ENTRY_COUNT: usize = 256;

/// CPU exception vectors.
pub mod vector {
    /// #DE: divide error.
    pub const DIVIDE_ERROR: u8 = 0;
    /// #DB: debug exception.
    pub const DEBUG: u8 = 1;
    /// #NM: non-maskable interrupt.
    pub const NON_MASKABLE_INTERRUPT: u8 = 2;
    /// #BP: breakpoint.
    pub const BREAKPOINT: u8 = 3;
    /// #OF: overflow.
    pub const OVERFLOW: u8 = 4;
    /// #BR: BOUND range exceeded.
    pub const BOUND_RANGE_EXCEEDED: u8 = 5;
    /// #UD: invalid opcode.
    pub const INVALID_OPCODE: u8 = 6;
    /// #DN: device not available (coprocessor).
    pub const DEVICE_NOT_AVAILABLE: u8 = 7;
    /// #DF: double fault.
    pub const DOUBLE_FAULT: u8 = 8;
    /// #CS: coprocessor segment override.
    pub const COPROCESSOR_SEGMENT_OVERRIDE: u8 = 9;
    /// #TS: invalid TSS.
    pub const INVALID_TSS: u8 = 10;
    /// #NP: segment not present.
    pub const SEGMENT_NOT_PRESENT: u8 = 11;
    /// #SS: stack-segment fault.
    pub const STACK_SEGMENT_FAULT: u8 = 12;
    /// #GP: general protection fault.
    pub const GENERAL_PROTECTION_FAULT: u8 = 13;
    /// #PF: page fault.
    pub const PAGE_FAULT: u8 = 14;
    /// Reserved by Intel.
    pub const RESERVED_15: u8 = 15;
    /// #MF: x87 floating-point exception.
    pub const X87_FLOATING_POINT: u8 = 16;
    /// #AC: alignment check.
    pub const ALIGNMENT_CHECK: u8 = 17;
    /// #MC: machine check.
    pub const MACHINE_CHECK: u8 = 18;
    /// #XM: SIMD floating-point exception.
    pub const SIMD_FLOATING_POINT: u8 = 19;
    /// #VE: virtualization exception.
    pub const VIRTUALIZATION_EXCEPTION: u8 = 20;
    /// #CP: control protection exception.
    pub const CONTROL_PROTECTION_EXCEPTION: u8 = 21;
}

/// Whether the given vector carries an error code.
///
/// The CPU pushes an error code onto the stack for a fixed set of exceptions.
/// For the remaining vectors the interrupt stub pushes a zero so that the
/// captured register state has the same layout for every vector.
pub const fn has_error_code(vector: u8) -> bool {
    matches!(
        vector,
        vector::DOUBLE_FAULT
            | vector::INVALID_TSS
            | vector::SEGMENT_NOT_PRESENT
            | vector::STACK_SEGMENT_FAULT
            | vector::GENERAL_PROTECTION_FAULT
            | vector::PAGE_FAULT
            | vector::ALIGNMENT_CHECK
            | vector::CONTROL_PROTECTION_EXCEPTION
    )
}

/// The name of a CPU exception.
///
/// Returns `None` for vectors that are not CPU exceptions.
pub const fn exception_name(vector: u8) -> Option<&'static str> {
    match vector {
        vector::DIVIDE_ERROR => Some("Divide Error"),
        vector::DEBUG => Some("Debug"),
        vector::NON_MASKABLE_INTERRUPT => Some("Non-Maskable Interrupt"),
        vector::BREAKPOINT => Some("Breakpoint"),
        vector::OVERFLOW => Some("Overflow"),
        vector::BOUND_RANGE_EXCEEDED => Some("BOUND Range Exceeded"),
        vector::INVALID_OPCODE => Some("Invalid Opcode"),
        vector::DEVICE_NOT_AVAILABLE => Some("Device Not Available"),
        vector::DOUBLE_FAULT => Some("Double Fault"),
        vector::COPROCESSOR_SEGMENT_OVERRIDE => Some("Coprocessor Segment Override"),
        vector::INVALID_TSS => Some("Invalid TSS"),
        vector::SEGMENT_NOT_PRESENT => Some("Segment Not Present"),
        vector::STACK_SEGMENT_FAULT => Some("Stack-Segment Fault"),
        vector::GENERAL_PROTECTION_FAULT => Some("General Protection Fault"),
        vector::PAGE_FAULT => Some("Page Fault"),
        vector::RESERVED_15 => Some("Reserved"),
        vector::X87_FLOATING_POINT => Some("x87 Floating-Point Exception"),
        vector::ALIGNMENT_CHECK => Some("Alignment Check"),
        vector::MACHINE_CHECK => Some("Machine Check"),
        vector::SIMD_FLOATING_POINT => Some("SIMD Floating-Point Exception"),
        vector::VIRTUALIZATION_EXCEPTION => Some("Virtualization Exception"),
        vector::CONTROL_PROTECTION_EXCEPTION => Some("Control Protection Exception"),
        _ => None,
    }
}

/// Whether the vector is one of the architecturally reserved vectors.
///
/// Reserved vectors must never be raised by hardware. A handler for them
/// exists anyway, so that a reserved vector is reported instead of silently
/// taking the machine down in an undefined way.
pub const fn is_reserved(vector: u8) -> bool {
    matches!(vector, 9 | 15 | 22..=31)
}

/// Whether the vector is a CPU exception (0..=31).
pub const fn is_exception(vector: u8) -> bool {
    vector < EXCEPTION_VECTOR_COUNT
}

/// Whether the vector belongs to a hardware interrupt.
pub const fn is_hardware_interrupt(vector: u8) -> bool {
    vector >= HARDWARE_VECTOR_BASE && vector <= HARDWARE_VECTOR_LAST
}

/// Bits of the page-fault error code.
pub mod page_fault {
    /// The page was not present (0) or the access violated protection (1).
    pub const PRESENT: u64 = 1 << 0;
    /// The access was a write (1) or a read (0).
    pub const WRITE: u64 = 1 << 1;
    /// The access came from user mode (1) or from the kernel (0).
    pub const USER: u64 = 1 << 2;
    /// The reserved bit of the page was set (1).
    pub const RESERVED_WRITE: u64 = 1 << 3;
    /// The access was an instruction fetch (1).
    pub const INSTRUCTION_FETCH: u64 = 1 << 4;
    /// The access was a shadow-stack access (1).
    pub const SHADOW_STACK: u64 = 1 << 5;
}

/// Bits of the general-protection, segment and TSS error codes.
///
/// The error code of these exceptions carries the selector that caused the
/// fault in its low sixteen bits, and three flags in the high bits:
///
/// ```text
///  19 18 17 16 15                                       0
///  L  I  E  0  selector index
/// ```
pub mod protection {
    /// The error code refers to an external source (1) or to an internal one (0).
    pub const EXTERNAL: u64 = 1 << 17;
    /// The selector refers to the interrupt descriptor table (1) or to the
    /// global descriptor table (0).
    pub const IDT: u64 = 1 << 18;
    /// The selector came from the local descriptor table (1) or from the
    /// global one (0).
    pub const LDT: u64 = 1 << 19;

    /// Mask of the selector index in the error code.
    pub const SELECTOR_INDEX_MASK: u64 = 0xFFFF;

    /// The table the selector came from.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Table {
        /// Global descriptor table.
        Gdt,
        /// Interrupt descriptor table.
        Idt,
        /// Local descriptor table.
        Ldt,
    }

    /// Decode the table the selector came from.
    ///
    /// A selector that refers to an interrupt descriptor table entry is
    /// reported as such; otherwise the source flag decides between the
    /// global and the local descriptor table.
    pub const fn table(error_code: u64) -> Table {
        if error_code & IDT != 0 {
            Table::Idt
        } else if error_code & LDT != 0 {
            Table::Ldt
        } else {
            Table::Gdt
        }
    }

    /// Decode the selector index of an error code.
    pub const fn selector_index(error_code: u64) -> u16 {
        (error_code & SELECTOR_INDEX_MASK) as u16
    }
}

/// The register state captured when the CPU delivers an event.
///
/// The layout matches the stack the CPU and the interrupt stub build. In
/// 64-bit mode the processor always pushes the stack segment and the stack
/// pointer, so the frame has the same size for every event:
///
/// ```text
/// low  ┌───────────────┐
///      │ general       │  pushed by the stub
///      │ registers     │
///      ├───────────────┤
///      │ vector        │  pushed by the stub
///      ├───────────────┤
///      │ error code    │  pushed by the processor (or a zero by the stub)
///      ├───────────────┤
///      │ rip           │  pushed by the processor
///      │ cs            │
///      │ rflags        │
///      │ rsp           │
///      │ ss            │
/// high └───────────────┘
/// ```
///
/// The processor pushes the error code last, so it ends up below the
/// instruction pointer. The stub pushes a placeholder in the place of the
/// error code for the vectors that carry none, which keeps the layout the
/// same for every vector.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct InterruptFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,
    /// The vector number.
    pub vector: u64,
    /// The error code, or zero for the vectors that carry none.
    pub error_code: u64,
    /// The instruction pointer of the interrupted code.
    pub rip: u64,
    /// The code segment of the interrupted code.
    pub cs: u64,
    /// The flags of the interrupted code.
    pub rflags: u64,
    /// The stack pointer of the interrupted code.
    pub rsp: u64,
    /// The stack segment of the interrupted code.
    pub ss: u64,
}

impl InterruptFrame {
    /// The vector number of the event.
    pub fn vector(&self) -> u8 {
        self.vector as u8
    }

    /// The error code of the event.
    ///
    /// The vectors that do not carry an error code report zero.
    pub fn error_code(&self) -> u64 {
        self.error_code
    }

    /// The instruction pointer of the interrupted code.
    pub fn rip(&self) -> u64 {
        self.rip
    }

    /// The stack pointer of the interrupted code.
    pub fn rsp(&self) -> u64 {
        self.rsp
    }

    /// The code segment of the interrupted code.
    pub fn cs(&self) -> u64 {
        self.cs
    }

    /// The flags of the interrupted code.
    pub fn rflags(&self) -> u64 {
        self.rflags
    }

    /// Whether the interrupted code ran in user mode.
    ///
    /// The code segment of the interrupted code carries the privilege level
    /// in its two lowest bits.
    pub fn from_user_mode(&self) -> bool {
        self.cs & 0x3 == 3
    }
}
