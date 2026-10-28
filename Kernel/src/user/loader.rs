//! Loading a program from the filesystem into the user window.
//!
//! The loader is the bridge between the filesystem and the user mode: it
//! reads a program image out of a file, checks it, and puts it into the
//! address space of the user program.
//!
//! # The steps
//!
//! 1. The path is resolved: a path without a separator is looked up in
//!    `/bin` first and in the root afterwards.
//! 2. The file is read and its header is checked.
//! 3. The payload is copied into the user code page, and the page becomes
//!    read-only again.
//! 4. The arguments are written into the user data page, and the argument
//!    vector is built on the user stack.
//!
//! # One window, many programs
//!
//! The kernel owns exactly one user window, so only one program can be
//! resident at a time. A program that starts another one therefore saves
//! the window first: [`save_window`] copies the code page, the data page and
//! the stack into the kernel heap, and [`restore_window`] writes them back
//! when the child program ended. The caller of the child program finds
//! itself exactly where it was.
//!
//! # Limitations
//!
//! - The image has to fit into the user code page. There is no demand
//!   paging and no second code page.
//! - The program is loaded at the base of the code page. There is no
//!   relocation, so the image only runs at that address.
//! - The arguments have to fit into the user data page.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ptr::NonNull;

use crate::fs::{FileSystem, FsError, OpenFlags};
use crate::user::address::{
    USER_CODE_BASE, USER_CODE_FLAGS, USER_CODE_PAGES, USER_DATA_BASE, USER_DATA_PAGES,
    USER_STACK_BASE, USER_STACK_PAGES, USER_STACK_TOP,
};
use crate::user::argv::Arguments;
use crate::user::image::{ImageError, ImageHeader};
use crate::vmm::mapper::{MapError, update_flags_current};
use crate::vmm::page_table::PAGE_SIZE;

/// The directory a program is looked up in when its name has no separator.
pub const PROGRAM_DIRECTORY: &str = "/bin";

/// The reasons a program cannot be loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    /// There is no such file.
    NotFound,
    /// The path names a directory.
    NotAFile,
    /// The program may not be read or run.
    PermissionDenied,
    /// The medium failed.
    IoError,
    /// The image is not a program image.
    InvalidImage(ImageError),
    /// The program does not fit into the user code page.
    ProgramTooLarge,
    /// The arguments do not fit into the user data page.
    ArgumentsTooLarge,
    /// The kernel heap could not provide the memory the loader needs.
    OutOfMemory,
    /// The virtual-memory subsystem refused a change of the page-table
    /// entries.
    Mapping(MapError),
    /// The path is not a valid path.
    InvalidPath,
}

impl From<ImageError> for LoadError {
    fn from(err: ImageError) -> Self {
        LoadError::InvalidImage(err)
    }
}

impl From<MapError> for LoadError {
    fn from(err: MapError) -> Self {
        LoadError::Mapping(err)
    }
}

impl From<FsError> for LoadError {
    fn from(err: FsError) -> Self {
        match err {
            FsError::NotFound => LoadError::NotFound,
            FsError::IsADirectory => LoadError::NotAFile,
            FsError::PermissionDenied => LoadError::PermissionDenied,
            FsError::InvalidPath => LoadError::InvalidPath,
            _ => LoadError::IoError,
        }
    }
}

/// A program that is ready to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadedProgram {
    /// The address of the first instruction of the program.
    pub entry: u64,
    /// The number of arguments.
    pub argc: u64,
    /// The address of the table of the arguments.
    pub argv: u64,
    /// The address of the block of the argument strings.
    pub block: u64,
    /// The size of the block of the argument strings.
    pub block_length: u64,
    /// The stack pointer the program starts with.
    pub stack_pointer: u64,
}

/// Resolve the path of a program.
///
/// A path that holds a separator is used as it is. A path without a
/// separator is looked up in [`PROGRAM_DIRECTORY`] first and in the root
/// afterwards, so that `hello` finds `/bin/hello`.
pub fn resolve_path(fs: &mut dyn FileSystem, path: &str) -> Result<String, LoadError> {
    if path.is_empty() {
        return Err(LoadError::InvalidPath);
    }
    if path.contains('/') {
        return Ok(path.to_string());
    }
    let in_bin = format!("{PROGRAM_DIRECTORY}/{path}");
    if fs.exists(&in_bin) {
        return Ok(in_bin);
    }
    let in_root = format!("/{path}");
    if fs.exists(&in_root) {
        return Ok(in_root);
    }
    Err(LoadError::NotFound)
}

/// Read a program out of the filesystem.
///
/// The function resolves the path of the program and reads its image. The
/// path it returns is the path the program was found under, and it is the
/// path the program sees as its first argument.
pub fn read_program(fs: &mut dyn FileSystem, path: &str) -> Result<(String, Vec<u8>), LoadError> {
    let resolved = resolve_path(fs, path)?;
    let image = read_image(fs, &resolved)?;
    Ok((resolved, image))
}

/// Read a program image out of the filesystem.
///
/// The path is used as it is: the caller resolves it before, because the
/// resolved path is the name of the program itself.
pub fn read_image(fs: &mut dyn FileSystem, path: &str) -> Result<Vec<u8>, LoadError> {
    let handle = fs.open(path, OpenFlags::read())?;
    let mut image = Vec::new();
    let mut buffer = [0u8; 512];
    loop {
        let count = fs.read(handle, &mut buffer)?;
        if count == 0 {
            break;
        }
        image.extend_from_slice(&buffer[..count]);
        if image.len() > MAX_IMAGE_SIZE {
            fs.close(handle)?;
            return Err(LoadError::ProgramTooLarge);
        }
    }
    fs.close(handle)?;
    // The header is checked before the caller gets the image: a file that
    // does not hold a program is refused here.
    ImageHeader::parse(&image)?;
    Ok(image)
}

/// The largest program image the loader accepts.
pub const MAX_IMAGE_SIZE: usize = 1024 * 1024;

/// Put a program into the user window.
///
/// The function writes the payload of the image into the code page, the
/// arguments into the data page, and the argument vector onto the stack. The
/// code page is read-only again when the function returns.
///
/// The function does not need the mapper: the user window is mapped once, at
/// boot, and the loader only changes the permission bits of the code page.
pub fn install(image: &[u8], arguments: &Arguments) -> Result<LoadedProgram, LoadError> {
    let header = ImageHeader::parse(image)?;
    let payload = header.payload(image);
    let code_size = USER_CODE_PAGES as u64 * PAGE_SIZE;
    if payload.len() as u64 > code_size {
        return Err(LoadError::ProgramTooLarge);
    }

    let block_size = arguments.block_size();
    let table_offset = align_up_bytes(block_size, 8);
    let total = table_offset + arguments.table_size();
    if total as u64 > USER_DATA_PAGES as u64 * PAGE_SIZE {
        return Err(LoadError::ArgumentsTooLarge);
    }

    // The code page has to be writable while the program is copied into it.
    update_flags_current(USER_CODE_BASE, crate::user::USER_CODE_WRITE_FLAGS)?;

    // SAFETY: the user code and data pages are mapped and writable, the
    // payload fits into the code page, and the arguments fit into the data
    // page.
    unsafe {
        core::ptr::copy_nonoverlapping(payload.as_ptr(), USER_CODE_BASE as *mut u8, payload.len());
        // The rest of the page is cleared, so that the program never sees
        // the remains of the program that was there before.
        core::ptr::write_bytes(
            (USER_CODE_BASE + payload.len() as u64) as *mut u8,
            0,
            (code_size as usize) - payload.len(),
        );
    }

    // The code page becomes read-only: the program may read it and run it,
    // but it cannot write to it.
    update_flags_current(USER_CODE_BASE, USER_CODE_FLAGS)?;

    let block = USER_DATA_BASE;
    let argv = USER_DATA_BASE + table_offset as u64;

    // SAFETY: the data page is mapped and writable, and the arguments were
    // checked against its size.
    unsafe {
        let mut offset = 0usize;
        let mut addresses: Vec<u64> = Vec::with_capacity(arguments.len());
        for entry in arguments.entries() {
            let bytes = entry.as_bytes();
            let address = block + offset as u64;
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), address as *mut u8, bytes.len());
            offset += bytes.len();
            // The NUL byte that ends the string.
            *((address + bytes.len() as u64) as *mut u8) = 0;
            offset += 1;
            addresses.push(address);
        }
        // The NUL byte that ends the block of the strings.
        *((block + offset as u64) as *mut u8) = 0;
        for (index, address) in addresses.iter().enumerate() {
            core::ptr::write_unaligned((argv + (index * 8) as u64) as *mut u64, *address);
        }
        // The null pointer that ends the table.
        core::ptr::write_unaligned((argv + (arguments.len() * 8) as u64) as *mut u64, 0u64);
    }

    // The argument vector is built on the stack: the number of the
    // arguments, the pointers to them, and the two null pointers that end
    // the vector and the auxiliary vector.
    let argc = arguments.len() as u64;
    let frame_size = align_up(16 + (arguments.len() * 8) as u64, 16);
    let stack_pointer = USER_STACK_TOP - frame_size;

    // SAFETY: the stack pages are mapped and writable, and the frame fits
    // into the stack.
    unsafe {
        let frame = stack_pointer as *mut u64;
        core::ptr::write_unaligned(frame, argc);
        for index in 0..arguments.len() {
            let address = core::ptr::read_unaligned((argv + (index * 8) as u64) as *const u64);
            core::ptr::write_unaligned(frame.add(1 + index), address);
        }
        core::ptr::write_unaligned(frame.add(1 + arguments.len()), 0u64);
        core::ptr::write_unaligned(frame.add(2 + arguments.len()), 0u64);
    }

    Ok(LoadedProgram {
        entry: USER_CODE_BASE + header.entry_offset as u64,
        argc,
        argv,
        block,
        block_length: block_size as u64,
        stack_pointer,
    })
}

/// Round a number up to the next multiple of sixteen.
fn align_up(value: u64, alignment: u64) -> u64 {
    value.div_ceil(alignment) * alignment
}

/// Round a number of bytes up to the next multiple of the alignment.
fn align_up_bytes(value: usize, alignment: usize) -> usize {
    value.div_ceil(alignment) * alignment
}

/// A copy of the user window.
///
/// The copy holds the code page, the data page and the stack of the user
/// window, in that order.
pub struct WindowImage {
    buffer: NonNull<u8>,
    size: usize,
}

/// The size of a copy of the user window.
pub const WINDOW_SIZE: usize =
    ((USER_CODE_PAGES + USER_DATA_PAGES + USER_STACK_PAGES) as u64 * PAGE_SIZE) as usize;

impl WindowImage {
    /// The bytes of the copy.
    fn bytes(&self) -> &[u8] {
        // SAFETY: the buffer is allocated with the size `size`, and the
        // window owns it for as long as the image exists.
        unsafe { core::slice::from_raw_parts(self.buffer.as_ptr(), self.size) }
    }
}

impl Drop for WindowImage {
    fn drop(&mut self) {
        let _ = crate::heap::dealloc(self.buffer, self.size);
    }
}

/// Copy the user window into the kernel heap.
///
/// The function is used before a program starts another one: the child
/// program overwrites the window, and the copy holds the state of the
/// parent until the child program ended.
pub fn save_window() -> Result<WindowImage, LoadError> {
    let block = crate::heap::alloc(WINDOW_SIZE, 16).map_err(|_| LoadError::OutOfMemory)?;
    let image = WindowImage {
        buffer: block,
        size: WINDOW_SIZE,
    };

    let sources = [
        (
            USER_CODE_BASE,
            (USER_CODE_PAGES as u64 * PAGE_SIZE) as usize,
        ),
        (
            USER_DATA_BASE,
            (USER_DATA_PAGES as u64 * PAGE_SIZE) as usize,
        ),
        (
            USER_STACK_BASE,
            (USER_STACK_PAGES as u64 * PAGE_SIZE) as usize,
        ),
    ];
    let mut offset = 0usize;
    for (base, length) in sources {
        // SAFETY: the pages of the user window are mapped, and the copy has
        // room for them.
        unsafe {
            core::ptr::copy_nonoverlapping(
                base as *const u8,
                image.buffer.as_ptr().add(offset),
                length,
            );
        }
        offset += length;
    }
    Ok(image)
}

/// Write a copy of the user window back into the window.
///
/// The code page is made writable for the copy and read-only again
/// afterwards, so that the program cannot change its own code.
pub fn restore_window(image: &WindowImage) -> Result<(), LoadError> {
    update_flags_current(USER_CODE_BASE, crate::user::USER_CODE_WRITE_FLAGS)?;

    let destinations = [
        (
            USER_CODE_BASE,
            (USER_CODE_PAGES as u64 * PAGE_SIZE) as usize,
        ),
        (
            USER_DATA_BASE,
            (USER_DATA_PAGES as u64 * PAGE_SIZE) as usize,
        ),
        (
            USER_STACK_BASE,
            (USER_STACK_PAGES as u64 * PAGE_SIZE) as usize,
        ),
    ];
    let bytes = image.bytes();
    let mut offset = 0usize;
    for (base, length) in destinations {
        // SAFETY: the pages of the user window are mapped, and the copy
        // holds exactly their content.
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes[offset..offset + length].as_ptr(),
                base as *mut u8,
                length,
            );
        }
        offset += length;
    }

    update_flags_current(USER_CODE_BASE, USER_CODE_FLAGS)?;
    Ok(())
}
