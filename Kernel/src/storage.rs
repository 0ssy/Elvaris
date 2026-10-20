//! The storage of the kernel: the filesystem the user programs see.
//!
//! The kernel owns exactly one filesystem. It is created when the kernel
//! starts, it holds the programs of the machine, and every system call that
//! touches a file goes through it.
//!
//! # The lock
//!
//! The filesystem is behind a lock, because a system call may arrive while
//! the kernel is doing something else with the filesystem. The lock is not
//! recursive: a program that runs while the filesystem is locked would
//! deadlock, so the kernel always releases the lock before a program runs.

use spin::Mutex;

use crate::fs::{FileSystem, FsError, MemFs};

/// The filesystem of the machine.
static FILESYSTEM: Mutex<Option<MemFs>> = Mutex::new(None);

/// The directories the filesystem of the machine holds.
pub const DIRECTORIES: [&str; 4] = ["/bin", "/dev", "/etc", "/tmp"];

/// Build the filesystem of the machine and mount it.
///
/// The function creates the directories of the system and installs the
/// programs of the initial userland.
pub fn initialize() -> Result<(), FsError> {
    let mut fs = MemFs::new();
    fs.mount()?;
    for directory in DIRECTORIES {
        fs.create_dir(directory, crate::fs::Permissions::directory())?;
    }
    crate::user::programs::install(&mut fs)?;
    *FILESYSTEM.lock() = Some(fs);
    Ok(())
}

/// Run a function with the filesystem of the machine.
///
/// The function fails when the filesystem is not mounted. The lock is held
/// while the function runs, so the function must not start a program: the
/// program would ask for the filesystem again.
pub fn with_filesystem<R>(f: impl FnOnce(&mut MemFs) -> R) -> Result<R, FsError> {
    let mut guard = FILESYSTEM.lock();
    let fs = guard.as_mut().ok_or(FsError::NotMounted)?;
    Ok(f(fs))
}

/// Whether the filesystem of the machine is mounted.
pub fn is_mounted() -> bool {
    FILESYSTEM
        .lock()
        .as_ref()
        .map(|fs| fs.is_mounted())
        .unwrap_or(false)
}

/// The number of files the filesystem of the machine holds.
pub fn file_count() -> usize {
    match with_filesystem(|fs| fs.read_dir("/").map(|entries| entries.len()).unwrap_or(0)) {
        Ok(count) => count,
        Err(_) => 0,
    }
}
