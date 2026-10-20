//! Unit tests for the pure parts of the user-mode subsystem.
//!
//! The tests cover the layout of the user address space, the validation of
//! the pointers a user program hands to the kernel, and the interpretation of
//! the permission bits of a page table entry. They run on the host, without
//! any hardware.

use super::address::{
    REGIONS, USER_ADDRESS_SPACE_END, USER_ADDRESS_SPACE_START, USER_CODE_BASE, USER_CODE_FLAGS,
    USER_DATA_BASE, USER_STACK_BASE, USER_STACK_PAGES, USER_STACK_TOP, UserRegion, is_user_address,
};
use super::argv::{Arguments, MAX_ARGUMENT_LENGTH, MAX_ARGUMENTS, tokenize};
use super::image::{self, IMAGE_HEADER_SIZE, IMAGE_MAGIC, ImageError, ImageHeader};
use super::loader;
use super::pointer::{PointerError, check_page_flags, page_count, validate_range};
use super::program;
use super::session::{self, USER_RFLAGS, USER_RPL};
use super::syscall::{MAX_WRITE_LENGTH, SYS_EXIT, SYS_WRITE};
use crate::cpu::gdt::{USER_CODE_SELECTOR, USER_DATA_SELECTOR};
use crate::fs::{FileSystem, MemFs, OpenFlags, Permissions};
use crate::vmm::page_table::entry_flags;

#[test]
fn the_user_address_space_is_a_window_in_the_low_half() {
    // The window is well below the kernel heap and the direct map.
    assert!(USER_ADDRESS_SPACE_START <= 0x0000_0080_0000_0000);
    assert!(USER_ADDRESS_SPACE_END < 0xFFFF_8000_0000_0000);
    assert!(USER_ADDRESS_SPACE_START < USER_ADDRESS_SPACE_END);
}

#[test]
fn the_user_regions_do_not_overlap() {
    let mut previous_end = USER_ADDRESS_SPACE_START;
    for region in REGIONS {
        assert_eq!(
            region.base(),
            previous_end,
            "the regions have to be contiguous and in ascending order"
        );
        previous_end = region.end();
    }
    assert_eq!(previous_end, USER_ADDRESS_SPACE_END);
}

#[test]
fn the_stack_grows_downwards_from_its_top() {
    assert_eq!(
        USER_STACK_TOP,
        USER_STACK_BASE + USER_STACK_PAGES as u64 * 4096
    );
    // The top of the stack is the first address above the stack region.
    assert!(is_user_address(USER_STACK_TOP - 1));
    assert!(!is_user_address(USER_STACK_TOP));
}

#[test]
fn the_user_window_holds_the_code_the_data_and_the_stack() {
    assert!(is_user_address(USER_CODE_BASE));
    assert!(is_user_address(USER_DATA_BASE));
    assert!(is_user_address(USER_STACK_TOP - 1));
    assert!(!is_user_address(0));
    assert!(!is_user_address(USER_ADDRESS_SPACE_START - 1));
    assert!(!is_user_address(USER_ADDRESS_SPACE_END));
}

#[test]
fn the_code_page_is_executable_and_the_data_pages_are_not() {
    // The code page carries no execute-disable bit, so the processor runs
    // the user program from it.
    assert_eq!(USER_CODE_FLAGS & entry_flags::NO_EXECUTE, 0);
    assert_ne!(USER_CODE_FLAGS & entry_flags::USER_ACCESSIBLE, 0);
    // The code page is read-only: the program cannot rewrite itself.
    assert_eq!(USER_CODE_FLAGS & entry_flags::WRITABLE, 0);
    // The data page and the stack are writable and not executable.
    let data_flags = super::address::USER_DATA_FLAGS;
    assert_ne!(data_flags & entry_flags::WRITABLE, 0);
    assert_ne!(data_flags & entry_flags::USER_ACCESSIBLE, 0);
    assert_ne!(data_flags & entry_flags::NO_EXECUTE, 0);
}

#[test]
fn kernel_pages_are_not_accessible_from_user_mode() {
    // The kernel heap, the direct map and the device registers all live
    // outside the user window, and none of them is mapped for user mode.
    assert!(!is_user_address(super::super::heap::HEAP_VIRT_BASE));
    assert!(!is_user_address(0xFFFF_8000_0000_0000));
    assert!(!is_user_address(crate::cpu::apic::LAPIC_VIRTUAL_BASE));
}

#[test]
fn a_range_inside_the_window_is_accepted() {
    assert_eq!(validate_range(USER_CODE_BASE, 16), Ok(()));
    assert_eq!(validate_range(USER_STACK_TOP - 8, 8), Ok(()));
}

#[test]
fn a_range_outside_the_window_is_rejected() {
    // A pointer into the kernel is rejected, whatever it points at.
    assert_eq!(
        validate_range(0xFFFF_8000_0000_0000, 8),
        Err(PointerError::OutsideUserSpace)
    );
    // The null page is not mapped for user mode either.
    assert_eq!(validate_range(0, 8), Err(PointerError::OutsideUserSpace));
    // A range that starts inside the window but leaves it is rejected.
    assert_eq!(
        validate_range(USER_ADDRESS_SPACE_END - 4, 8),
        Err(PointerError::OutsideUserSpace)
    );
}

#[test]
fn a_range_that_overflows_is_rejected() {
    // The length is added to the address; the sum has to stay inside the
    // window, and an addition that wraps around is never valid.
    assert_eq!(validate_range(u64::MAX - 3, 8), Err(PointerError::Overflow));
    assert_eq!(
        validate_range(USER_ADDRESS_SPACE_END - 1, u64::MAX),
        Err(PointerError::Overflow)
    );
    // An address that is not canonical is rejected before anything else: the
    // upper sixteen bits have to be all zero or all one.
    assert_eq!(
        validate_range(0x0000_8000_0000_0000, 8),
        Err(PointerError::NotCanonical)
    );
    assert_eq!(
        validate_range(0xFFFF_0000_0000_0000, 8),
        Err(PointerError::NotCanonical)
    );
}

#[test]
fn a_range_of_length_zero_is_accepted() {
    // A range of length zero describes no memory, so there is nothing the
    // kernel could dereference.
    assert_eq!(validate_range(0, 0), Ok(()));
    assert_eq!(validate_range(u64::MAX, 0), Ok(()));
}

#[test]
fn the_page_count_covers_both_ends_of_a_range() {
    assert_eq!(page_count(USER_CODE_BASE, 0), 0);
    assert_eq!(page_count(USER_CODE_BASE, 1), 1);
    assert_eq!(page_count(USER_CODE_BASE, 4096), 1);
    assert_eq!(page_count(USER_CODE_BASE, 4097), 2);
    // A range that starts in the middle of a page covers both pages.
    assert_eq!(page_count(USER_CODE_BASE + 4090, 10), 2);
}

#[test]
fn the_permission_bits_of_a_page_are_checked() {
    let user_readable = entry_flags::PRESENT | entry_flags::USER_ACCESSIBLE;
    let user_writable = user_readable | entry_flags::WRITABLE;

    assert_eq!(check_page_flags(user_readable, false), Ok(()));
    assert_eq!(
        check_page_flags(user_readable, true),
        Err(PointerError::NotWritable)
    );
    assert_eq!(check_page_flags(user_writable, true), Ok(()));
    // A kernel page is present but not accessible from user mode.
    assert_eq!(
        check_page_flags(entry_flags::PRESENT, false),
        Err(PointerError::NotUserAccessible)
    );
    // A page that is not present cannot be used at all.
    assert_eq!(check_page_flags(0, false), Err(PointerError::Unmapped));
}

#[test]
fn the_user_segments_carry_the_privilege_level_of_user_mode() {
    // The kernel enters user mode with the user segments, and the requested
    // privilege level of both is three.
    assert_eq!(USER_CODE_SELECTOR | USER_RPL, 0x1B);
    assert_eq!(USER_DATA_SELECTOR | USER_RPL, 0x23);
    assert_eq!(USER_CODE_SELECTOR & USER_RPL, 0);
    assert_eq!(USER_DATA_SELECTOR & USER_RPL, 0);
}

#[test]
fn the_user_program_starts_with_the_interrupts_enabled() {
    // Bit one of the flags always reads as one.
    assert_eq!(USER_RFLAGS & 0x2, 0x2);
    // The interrupt flag lets the processor deliver the timer interrupt to
    // the user program.
    assert_eq!(USER_RFLAGS & (1 << 9), 1 << 9);
}

#[test]
fn the_user_program_is_a_complete_image() {
    let image = program::image();
    // The image holds the two system calls and the instruction that ends the
    // program if the kernel ever returns from `exit`.
    assert!(image.len() > 16);
    // The system call instruction is `int 0x80`, which the assembler encodes
    // as CD 80.
    let mut found = false;
    for window in image.windows(2) {
        if window == [0xCD, 0x80] {
            found = true;
            break;
        }
    }
    assert!(
        found,
        "the user program has to use the system call instruction"
    );
    assert_eq!(program::entry_address(), USER_CODE_BASE);
}

#[test]
fn the_probes_follow_the_user_program_in_the_code_page() {
    // The probes live in the same image, so one copy of the code page holds
    // the user program and both probes.
    let main = program::entry_address();
    let fault = program::fault_probe_address();
    let unknown = program::unknown_syscall_probe_address();

    assert!(main < fault);
    assert!(fault < unknown);
    // All three fit into the single code page of the user address space.
    let end = unknown + 64;
    assert!(end <= USER_CODE_BASE + 4096);
    // The addresses are inside the user window.
    assert!(is_user_address(fault));
    assert!(is_user_address(unknown));
}

#[test]
fn the_fault_probe_reads_from_an_address_that_is_not_mapped() {
    // The probe reads from the first address above the user address space.
    // That address is not mapped, so the processor raises a page fault and
    // the kernel takes the program down.
    let probe_address = USER_ADDRESS_SPACE_END;
    assert!(!is_user_address(probe_address));
    assert_eq!(probe_address, USER_ADDRESS_SPACE_END);
}

#[test]
fn the_unknown_syscall_probe_uses_a_number_the_kernel_does_not_know() {
    // The kernel knows the numbers one and two only.
    let number = program::unknown_syscall_number();
    assert_ne!(number, SYS_WRITE);
    assert_ne!(number, SYS_EXIT);
}

#[test]
fn the_message_the_user_program_writes_is_not_empty() {
    assert!(!program::MESSAGE.is_empty());
    // The message ends with a newline, so the console stays readable.
    assert_eq!(program::MESSAGE.last(), Some(&b'\n'));
}

#[test]
fn the_system_call_numbers_are_stable() {
    assert_eq!(SYS_WRITE, 1);
    assert_eq!(SYS_EXIT, 2);
    // A single write cannot produce more than the limit.
    assert!(MAX_WRITE_LENGTH > 0);
}

#[test]
fn the_outcome_of_a_session_starts_out_empty() {
    // The session belongs to the kernel; a program that never ran leaves the
    // outcome at "running".
    assert_eq!(session::OUTCOME_RUNNING, 0);
    assert_ne!(session::OUTCOME_EXITED, session::OUTCOME_FAULTED);
    assert_ne!(session::OUTCOME_REJECTED, session::OUTCOME_EXITED);
}

#[test]
fn the_regions_describe_themselves() {
    let code = UserRegion::Code {
        base: USER_CODE_BASE,
        pages: 1,
    };
    assert_eq!(code.base(), USER_CODE_BASE);
    assert_eq!(code.pages(), 1);
    assert_eq!(code.end(), USER_CODE_BASE + 4096);
}

// ---- the format of a program image ----

#[test]
fn an_image_carries_its_payload_and_its_entry_point() {
    let payload = [0x90u8; 16];
    let bytes = image::build(&payload, 4);

    let header = ImageHeader::parse(&bytes).expect("the image is valid");
    assert_eq!(header.version, image::IMAGE_VERSION);
    assert_eq!(header.entry_offset, 4);
    assert_eq!(header.payload_size, payload.len() as u32);
    assert_eq!(header.payload(&bytes), &payload);
}

#[test]
fn an_image_that_is_too_short_is_refused() {
    let bytes = [0u8; 8];
    assert_eq!(ImageHeader::parse(&bytes), Err(ImageError::TooShort));
}

#[test]
fn an_image_without_the_magic_is_refused() {
    let mut bytes = image::build(&[0x90u8; 4], 0);
    bytes[0] = b'X';
    assert_eq!(ImageHeader::parse(&bytes), Err(ImageError::BadMagic));
    assert_eq!(bytes[0], b'X');
    assert_eq!(&IMAGE_MAGIC, b"ELVI");
}

#[test]
fn an_image_with_an_unknown_version_is_refused() {
    let mut bytes = image::build(&[0x90u8; 4], 0);
    bytes[4] = 99;
    assert_eq!(
        ImageHeader::parse(&bytes),
        Err(ImageError::UnsupportedVersion)
    );
}

#[test]
fn an_image_with_a_truncated_payload_is_refused() {
    let mut bytes = image::build(&[0x90u8; 32], 0);
    bytes.truncate(IMAGE_HEADER_SIZE + 8);
    assert_eq!(ImageHeader::parse(&bytes), Err(ImageError::PayloadTooLarge));
}

#[test]
fn an_image_with_an_entry_point_outside_the_payload_is_refused() {
    let bytes = image::build(&[0x90u8; 8], 8);
    assert_eq!(ImageHeader::parse(&bytes), Err(ImageError::EntryOutOfRange));
}

#[test]
fn an_image_without_a_payload_is_refused() {
    // An empty payload has no entry point, so the image cannot be run.
    let bytes = image::build(&[], 0);
    assert_eq!(ImageHeader::parse(&bytes), Err(ImageError::EntryOutOfRange));
}

// ---- the arguments of a program ----

#[test]
fn a_command_line_is_split_into_its_words() {
    let words = tokenize("one two   three");
    assert_eq!(words, vec!["one", "two", "three"]);
    // A tab separates the words as well.
    let words = tokenize("one\ttwo");
    assert_eq!(words, vec!["one", "two"]);
    // An empty command line has no words.
    assert!(tokenize("   ").is_empty());
}

#[test]
fn the_first_argument_of_a_program_is_its_path() {
    let arguments = Arguments::new("/bin/hello", "one two");
    assert_eq!(arguments.len(), 3);
    assert_eq!(arguments.get(0), Some("/bin/hello"));
    assert_eq!(arguments.get(1), Some("one"));
    assert_eq!(arguments.get(2), Some("two"));
    assert_eq!(arguments.get(3), None);
}

#[test]
fn a_program_without_arguments_gets_only_its_path() {
    let arguments = Arguments::new("/bin/hello", "");
    assert_eq!(arguments.len(), 1);
    assert_eq!(arguments.get(0), Some("/bin/hello"));
}

#[test]
fn the_arguments_of_a_program_are_cut_at_their_limits() {
    let long = "x".repeat(MAX_ARGUMENT_LENGTH + 10);
    let arguments = Arguments::new("/bin/hello", &long);
    assert_eq!(
        arguments.get(1).map(str::len),
        Some(MAX_ARGUMENT_LENGTH),
        "an argument may not be longer than the limit"
    );

    let many = (0..MAX_ARGUMENTS + 5)
        .map(|index| format!("word{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    let arguments = Arguments::new("/bin/hello", &many);
    assert_eq!(
        arguments.len(),
        MAX_ARGUMENTS,
        "a program may not get more arguments than the limit"
    );
}

#[test]
fn the_block_of_the_arguments_holds_every_string() {
    let arguments = Arguments::from_words(&["/bin/hello", "one", "two"]);
    // Every string ends with a NUL byte, and the block ends with one more.
    let expected = "/bin/hello".len() + 1 + "one".len() + 1 + "two".len() + 1 + 1;
    assert_eq!(arguments.block_size(), expected);
    // The table holds one pointer per argument and the null pointer that
    // ends it.
    assert_eq!(arguments.table_size(), (arguments.len() + 1) * 8);
}

// ---- the loader ----

/// A filesystem with a program in it.
fn filesystem_with_program() -> MemFs {
    let mut fs = MemFs::new();
    fs.mount().expect("the filesystem mounts");
    fs.create_dir("/bin", Permissions::directory())
        .expect("the directory is created");
    let payload = [0x90u8; 8];
    let bytes = image::build(&payload, 0);
    fs.create_file("/bin/hello", Permissions::program())
        .expect("the file is created");
    let handle = fs
        .open("/bin/hello", OpenFlags::write())
        .expect("the file opens");
    fs.write(handle, &bytes).expect("the image is written");
    fs.close(handle).expect("the file closes");
    fs
}

#[test]
fn a_program_without_a_separator_is_looked_up_in_the_program_directory() {
    let mut fs = filesystem_with_program();
    let path = loader::resolve_path(&mut fs, "hello").expect("the program is found");
    assert_eq!(path, "/bin/hello");
}

#[test]
fn a_program_with_a_separator_is_used_as_it_is() {
    let mut fs = filesystem_with_program();
    let path = loader::resolve_path(&mut fs, "/bin/hello").expect("the program is found");
    assert_eq!(path, "/bin/hello");
}

#[test]
fn a_program_that_does_not_exist_is_reported() {
    let mut fs = filesystem_with_program();
    assert_eq!(
        loader::resolve_path(&mut fs, "missing"),
        Err(loader::LoadError::NotFound)
    );
    assert_eq!(
        loader::resolve_path(&mut fs, ""),
        Err(loader::LoadError::InvalidPath)
    );
}

#[test]
fn the_loader_reads_the_image_of_a_program() {
    let mut fs = filesystem_with_program();
    let (path, bytes) = loader::read_program(&mut fs, "hello").expect("the program is read");
    assert_eq!(path, "/bin/hello");

    let header = ImageHeader::parse(&bytes).expect("the image is valid");
    assert_eq!(header.payload(&bytes), &[0x90u8; 8]);
}

#[test]
fn the_loader_refuses_a_file_that_is_not_a_program() {
    let mut fs = filesystem_with_program();
    fs.create_file("/notes", Permissions::system_write())
        .expect("the file is created");
    let handle = fs
        .open("/notes", OpenFlags::write())
        .expect("the file opens");
    fs.write(handle, b"not an image")
        .expect("the text is written");
    fs.close(handle).expect("the file closes");

    let error = loader::read_program(&mut fs, "/notes").expect_err("the file is refused");
    assert!(
        matches!(error, loader::LoadError::InvalidImage(_)),
        "the loader has to refuse a file that is not a program image"
    );
}
