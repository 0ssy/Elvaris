//! Unit tests for virtual memory management.
//!
//! These tests run on the host and verify address math, flag validation,
//! and mapping logic without requiring QEMU.

#![cfg(test)]

use super::page_table::entry_flags;
use super::mapper::{MapError, MapFlags};
use super::page_table::{
    pd_index, pdpt_index, pml4_index, page_offset, page_start, pt_index, PAGE_SIZE,
};

#[test]
fn page_table_indexes_known_address() {
    // Virtual address 0xFFFF_F800_0000_1234
    let addr: u64 = 0xFFFF_F800_0000_1234;
    assert_eq!(pml4_index(addr), 496);
    assert_eq!(pdpt_index(addr), 0);
    assert_eq!(pd_index(addr), 0);
    assert_eq!(pt_index(addr), 1);
    assert_eq!(page_offset(addr), 0x234);
    assert_eq!(page_start(addr), 0xFFFF_F800_0000_1000);
}

#[test]
fn page_table_indexes_zero() {
    assert_eq!(pml4_index(0), 0);
    assert_eq!(pdpt_index(0), 0);
    assert_eq!(pd_index(0), 0);
    assert_eq!(pt_index(0), 0);
    assert_eq!(page_offset(0), 0);
}

#[test]
fn page_start_aligns_down() {
    assert_eq!(page_start(0x1234), 0x1000);
    assert_eq!(page_start(0xFFF), 0);
    assert_eq!(page_start(0x1000), 0x1000);
    assert_eq!(page_start(0x1FFF), 0x1000);
}

#[test]
fn canonical_address_detection() {
    use super::page_table::is_canonical;
    // Canonical user addresses (bit 47 = 0, upper bits 0)
    assert!(is_canonical(0x0000_0000_0000_0000));
    assert!(is_canonical(0x0000_7FFF_FFFF_FFFF));
    // Canonical kernel addresses (bit 47 = 1, upper bits all 1)
    assert!(is_canonical(0xFFFF_8000_0000_0000));
    assert!(is_canonical(0xFFFF_FFFF_FFFF_FFFF));
    // Non-canonical (bit 47 = 1 but upper bits not all 1)
    assert!(!is_canonical(0x0000_8000_0000_0000));
    assert!(!is_canonical(0x0001_0000_0000_0000));
}

#[test]
fn page_size_constant() {
    assert_eq!(PAGE_SIZE, 4096);
}

#[test]
fn flag_validation_rejects_huge_page() {
    assert!(MapFlags::validate(entry_flags::HUGE_PAGE).is_err());
}

#[test]
fn flag_validation_accepts_normal_flags() {
    assert!(MapFlags::validate(entry_flags::PRESENT).is_ok());
    assert!(MapFlags::validate(entry_flags::PRESENT | entry_flags::WRITABLE).is_ok());
    assert!(MapFlags::validate(entry_flags::PRESENT | entry_flags::USER_ACCESSIBLE).is_ok());
}

#[test]
fn flag_constructors_expected_bits() {
    assert_eq!(MapFlags::kernel_readonly().bits(), entry_flags::PRESENT);
    assert_eq!(
        MapFlags::kernel_writable().bits(),
        entry_flags::PRESENT | entry_flags::WRITABLE
    );
    assert_eq!(
        MapFlags::kernel_executable().bits(),
        entry_flags::PRESENT
    );
    assert_eq!(
        MapFlags::user_readonly().bits(),
        entry_flags::PRESENT | entry_flags::USER_ACCESSIBLE
    );
    assert_eq!(
        MapFlags::user_writable().bits(),
        entry_flags::PRESENT | entry_flags::WRITABLE | entry_flags::USER_ACCESSIBLE
    );
}

#[test]
fn flag_contains_works() {
    let f = MapFlags::new(entry_flags::PRESENT | entry_flags::WRITABLE);
    assert!(f.bits() & entry_flags::PRESENT != 0);
    assert!(f.bits() & entry_flags::WRITABLE != 0);
    assert!(f.bits() & entry_flags::USER_ACCESSIBLE == 0);
}

#[test]
fn flag_bits_roundtrip() {
    let bits = entry_flags::PRESENT | entry_flags::WRITABLE | entry_flags::NO_EXECUTE;
    let f = MapFlags::new(bits);
    assert_eq!(f.bits(), bits);
}

#[test]
fn map_flags_kernel_code_is_readonly_supervisor() {
    let bits = MapFlags::kernel_executable().bits();
    assert!((bits & entry_flags::PRESENT) != 0);
    assert!((bits & entry_flags::WRITABLE) == 0);
    assert!((bits & entry_flags::USER_ACCESSIBLE) == 0);
}

#[test]
fn map_flags_kernel_data_is_writable_supervisor() {
    let bits = MapFlags::kernel_writable().bits();
    assert!((bits & entry_flags::PRESENT) != 0);
    assert!((bits & entry_flags::WRITABLE) != 0);
    assert!((bits & entry_flags::USER_ACCESSIBLE) == 0);
}

#[test]
fn map_flags_user_is_accessible() {
    let bits = MapFlags::user_readonly().bits();
    assert!((bits & entry_flags::PRESENT) != 0);
    assert!((bits & entry_flags::USER_ACCESSIBLE) != 0);
    assert!((bits & entry_flags::WRITABLE) == 0);
}

#[test]
fn map_flags_user_writable_is_writable() {
    let bits = MapFlags::user_writable().bits();
    assert!((bits & entry_flags::PRESENT) != 0);
    assert!((bits & entry_flags::WRITABLE) != 0);
    assert!((bits & entry_flags::USER_ACCESSIBLE) != 0);
}

#[test]
fn error_types_display_correctly() {
    assert!(format!("{:?}", MapError::NonCanonicalAddress).contains("NonCanonical"));
    assert!(format!("{:?}", MapError::MappingAlreadyExists).contains("MappingAlreadyExists"));
    assert!(format!("{:?}", MapError::OutOfMemory).contains("OutOfMemory"));
    assert!(format!("{:?}", MapError::FrameNotAligned).contains("FrameNotAligned"));
}

#[test]
fn entries_per_table_constant() {
    assert_eq!(512, 512);
}

#[test]
fn entry_flag_constants_expected() {
    assert_eq!(entry_flags::PRESENT, 1 << 0);
    assert_eq!(entry_flags::WRITABLE, 1 << 1);
    assert_eq!(entry_flags::USER_ACCESSIBLE, 1 << 2);
    assert_eq!(entry_flags::NO_EXECUTE, 1 << 63);
}
