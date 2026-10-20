//! The task state segment.
//!
//! The task state segment (TSS) describes the stack the processor switches to
//! when an event arrives from a less privileged level, and the interrupt stack
//! table (IST) that gives a handler its own stack.
//!
//! The layout is fixed by the hardware:
//!
//! ```text
//!  0x00  reserved
//!  0x04  privilege stack table   (RSP0, RSP1, RSP2)
//!  0x1C  reserved
//!  0x20  interrupt stack table   (IST1 .. IST7)
//!  0x58  reserved
//!  0x5C  I/O map base
//!  0x5E  I/O permission bitmap
//! ```
//!
//! The 64-bit fields are only four-byte aligned, which is why the structure is
//! packed.

use core::mem::offset_of;

/// Size of the stack used for double faults.
pub const IST_STACK_SIZE: usize = 16 * 1024;

/// Index of the interrupt stack table entry used for double faults.
///
/// Entry 0 means "use the current stack", so the dedicated stacks start at 1.
pub const DOUBLE_FAULT_IST: usize = 1;

/// The task state segment.
///
/// The structure is packed because the hardware places the 64-bit stacks at
/// four-byte-aligned offsets.
#[repr(C, packed)]
pub struct TaskStateSegment {
    reserved_0: u32,
    /// `RSP0` is the stack the processor loads when an event arrives from ring
    /// 3; `RSP1` and `RSP2` are reserved for the other user levels.
    pub privilege_stack_table: [u64; 3],
    reserved_1: u32,
    /// The stacks used by the interrupt stack table entries 1..=7.
    pub interrupt_stack_table: [u64; 7],
    reserved_2: u32,
    /// Offset of the I/O permission bitmap.
    pub iomap_base: u16,
    /// The I/O permission bitmap. A set bit denies the port.
    pub iomap: [u8; 8],
    /// Padding that brings the segment up to the 104 bytes the processor
    /// requires in long mode. The bytes deny the ports above the bitmap.
    padding: [u8; 2],
}

impl TaskStateSegment {
    /// A cleared task state segment with no usable stacks.
    pub const fn new() -> Self {
        Self {
            reserved_0: 0,
            privilege_stack_table: [0; 3],
            reserved_1: 0,
            interrupt_stack_table: [0; 7],
            reserved_2: 0,
            iomap_base: IOMAP_BASE,
            // Deny every port: the kernel does not use the I/O permission
            // bitmap, and a denied port is the safe default.
            iomap: [0xFF; 8],
            // The bytes that follow the bitmap deny the ports above it.
            padding: [0xFF; 2],
        }
    }

    /// The limit of the segment, in bytes.
    pub const fn limit(&self) -> u32 {
        core::mem::size_of::<TaskStateSegment>() as u32 - 1
    }
}

impl Default for TaskStateSegment {
    fn default() -> Self {
        Self::new()
    }
}

/// Offset of the I/O permission bitmap inside the segment.
const IOMAP_BASE: u16 = offset_of!(TaskStateSegment, iomap) as u16;

/// A stack with the alignment the processor expects for an interrupt stack.
#[repr(C, align(16))]
struct InterruptStack([u8; IST_STACK_SIZE]);

static mut IST1_STACK: InterruptStack = InterruptStack([0; IST_STACK_SIZE]);

/// The task state segment in an aligned wrapper.
///
/// The processor requires the base address of the task state segment to be
/// aligned on a sixteen-byte boundary. The structure itself is packed,
/// because the hardware places its 64-bit fields at four-byte-aligned
/// offsets.
#[repr(C, align(16))]
struct AlignedTaskStateSegment(TaskStateSegment);

static mut TSS: AlignedTaskStateSegment = AlignedTaskStateSegment(TaskStateSegment::new());

/// The top of the stack used for double faults.
///
/// The stack grows downwards, so the top is the end of the array.
pub fn double_fault_stack_top() -> u64 {
    let base = core::ptr::addr_of_mut!(IST1_STACK) as u64;
    base + IST_STACK_SIZE as u64
}

/// Prepare the task state segment.
///
/// `kernel_stack_top` is the top of the stack the processor must use when an
/// event arrives from user mode. It is taken from the boot information, which
/// describes the stack the bootloader prepared for the kernel.
///
/// The segment itself is loaded by [`crate::cpu::gdt::install`], because the
/// GDT holds its descriptor.
pub fn prepare(kernel_stack_top: u64) {
    let tss = unsafe { &mut *core::ptr::addr_of_mut!(TSS) };

    // Ring 0 stack. The entries for ring 1 and ring 2 stay zero: the kernel
    // does not support those privilege levels.
    tss.0.privilege_stack_table[0] = kernel_stack_top;

    // The dedicated double-fault stack.
    tss.0.interrupt_stack_table[DOUBLE_FAULT_IST - 1] = double_fault_stack_top();

    // Deny all port I/O.
    tss.0.iomap = [0xFF; 8];
}

/// The base address of the prepared task state segment.
pub fn base_address() -> u64 {
    core::ptr::addr_of!(TSS) as u64
}

/// The limit of the prepared task state segment.
pub fn limit() -> u32 {
    unsafe { (*core::ptr::addr_of!(TSS)).0.limit() }
}

/// The value of the ring 0 privilege stack.
pub fn kernel_stack_top() -> u64 {
    unsafe { (*core::ptr::addr_of!(TSS)).0.privilege_stack_table[0] }
}

/// The selector the task register currently holds.
///
/// The processor does not expose the base of the task state segment directly;
/// the selector is the visible part of it, and it is enough to confirm that
/// the segment was loaded.
pub fn task_register_selector() -> u16 {
    let selector: u16;
    unsafe {
        core::arch::asm!("str ax", out("ax") selector, options(nomem, nostack, preserves_flags));
    }
    selector
}
