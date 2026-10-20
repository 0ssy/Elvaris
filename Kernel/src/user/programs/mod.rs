//! The programs of the initial userland.
//!
//! The programs are machine code the kernel assembles itself. There is no
//! executable format yet: every program is a block of bytes with a fixed
//! entry convention, and the kernel builds the image of the program out of
//! it.
//!
//! # The programs
//!
//! | Path | Program |
//! |------|---------|
//! | `/init` | the first program of the machine: it starts the shell |
//! | `/bin/sh` | the shell |
//! | `/bin/hello` | a program that greets the person at the console |
//!
//! The programs are installed into the filesystem of the machine when the
//! kernel starts, so that the shell finds them like any other program.

pub mod hello;
pub mod init;
pub mod shell;

use crate::fs::{FileSystem, FsError, MemFs, OpenFlags, Permissions};
use crate::user::image;

/// The path of every program of the initial filesystem.
pub const PATHS: [&str; 3] = ["/init", "/bin/sh", "/bin/hello"];

/// The machine code of every program of the initial filesystem.
pub const PAYLOADS: [fn() -> &'static [u8]; 3] = [init::payload, shell::payload, hello::payload];

/// Install the programs into the filesystem.
///
/// The function creates the file of every program and writes its image into
/// it. The image is the header of the format and the machine code of the
/// program.
pub fn install(fs: &mut MemFs) -> Result<(), FsError> {
    for (path, payload) in PATHS.iter().zip(PAYLOADS.iter()) {
        install_program(fs, path, payload())?;
    }
    Ok(())
}

/// Put a program into the filesystem.
///
/// The function writes the image of the program into a file of the
/// filesystem. The program starts at the beginning of the payload, so the
/// offset of the entry point is zero.
pub fn install_program(fs: &mut MemFs, path: &str, payload: &[u8]) -> Result<(), FsError> {
    let image = image::build(payload, 0);
    fs.create_file(path, Permissions::program())?;
    let handle = fs.open(path, OpenFlags::write())?;
    fs.write(handle, &image)?;
    fs.close(handle)?;
    Ok(())
}
