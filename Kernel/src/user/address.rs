//! The layout of the user address space.
//!
//! The kernel and the user program share one page table hierarchy for now:
//! the kernel mappings stay supervisor-only, and the user mappings are the
//! only ones the processor lets user mode touch. The user range therefore
//! lives in a small, fixed window of the low half of the address space, far
//! away from the kernel heap, from the physical-memory direct map and from
//! the memory mapped device registers.
//!
//! ```text
//!  0x0000_0080_0000_0000  user code      1 page   read + execute
//!  0x0000_0080_0000_1000  user data      1 page   read + write, no execute
//!  0x0000_0080_0000_2000  user stack     4 pages  read + write, no execute
//!  0x0000_0080_0000_6000  end of the user address space
//! ```
//!
//! Everything below the window is left alone. The first page of the address
//! space is never mapped, so a null pointer in user code faults instead of
//! reaching the kernel.

use crate::vmm::page_table::{PAGE_SIZE, entry_flags};

/// The first address of the user address space.
pub const USER_ADDRESS_SPACE_START: u64 = 0x0000_0080_0000_0000;

/// The first address above the user address space.
///
/// The window is exclusive: an address equal to this value belongs to the
/// kernel, not to the user program.
pub const USER_ADDRESS_SPACE_END: u64 = 0x0000_0080_0000_6000;

/// Base address of the user code page.
pub const USER_CODE_BASE: u64 = USER_ADDRESS_SPACE_START;

/// Number of pages of user code.
pub const USER_CODE_PAGES: usize = 1;

/// Base address of the user data page.
pub const USER_DATA_BASE: u64 = 0x0000_0080_0000_1000;

/// Number of pages of user data.
pub const USER_DATA_PAGES: usize = 1;

/// Base address of the user stack.
pub const USER_STACK_BASE: u64 = 0x0000_0080_0000_2000;

/// Number of pages of the user stack.
pub const USER_STACK_PAGES: usize = 4;

/// Top of the user stack.
///
/// The stack grows downwards, so this is the first address the user program
/// sees as its stack pointer.
pub const USER_STACK_TOP: u64 = USER_STACK_BASE + USER_STACK_PAGES as u64 * PAGE_SIZE;

/// Page-table flags of the user code page.
///
/// The page is present and accessible from user mode, and it is executable:
/// the user program runs from it. It is not writable, so the user program
/// cannot rewrite its own code.
pub const USER_CODE_FLAGS: u64 = entry_flags::PRESENT | entry_flags::USER_ACCESSIBLE;

/// Page-table flags of the user data and stack pages.
///
/// The pages are present, writable and accessible from user mode. They carry
/// the no-execute bit: data must never be run as code, and the processor has
/// to enforce that.
pub const USER_DATA_FLAGS: u64 = entry_flags::PRESENT
    | entry_flags::WRITABLE
    | entry_flags::USER_ACCESSIBLE
    | entry_flags::NO_EXECUTE;

/// Whether the address belongs to the user address space.
pub const fn is_user_address(address: u64) -> bool {
    address >= USER_ADDRESS_SPACE_START && address < USER_ADDRESS_SPACE_END
}

/// The regions the user address space consists of.
///
/// The kernel reports the layout on the serial console and uses the regions
/// to map the address space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserRegion {
    /// The user code page.
    Code {
        /// Base address of the region.
        base: u64,
        /// Number of pages of the region.
        pages: usize,
    },
    /// The user data page.
    Data {
        /// Base address of the region.
        base: u64,
        /// Number of pages of the region.
        pages: usize,
    },
    /// The user stack.
    Stack {
        /// Base address of the region.
        base: u64,
        /// Number of pages of the region.
        pages: usize,
    },
}

impl UserRegion {
    /// The base address of the region.
    pub const fn base(&self) -> u64 {
        match *self {
            UserRegion::Code { base, .. } => base,
            UserRegion::Data { base, .. } => base,
            UserRegion::Stack { base, .. } => base,
        }
    }

    /// The number of pages of the region.
    pub const fn pages(&self) -> usize {
        match *self {
            UserRegion::Code { pages, .. } => pages,
            UserRegion::Data { pages, .. } => pages,
            UserRegion::Stack { pages, .. } => pages,
        }
    }

    /// The first address above the region.
    pub const fn end(&self) -> u64 {
        self.base() + self.pages() as u64 * PAGE_SIZE
    }
}

/// The regions of the user address space, in ascending order.
pub const REGIONS: [UserRegion; 3] = [
    UserRegion::Code {
        base: USER_CODE_BASE,
        pages: USER_CODE_PAGES,
    },
    UserRegion::Data {
        base: USER_DATA_BASE,
        pages: USER_DATA_PAGES,
    },
    UserRegion::Stack {
        base: USER_STACK_BASE,
        pages: USER_STACK_PAGES,
    },
];
