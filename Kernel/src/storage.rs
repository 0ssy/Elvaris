//! The storage of the kernel: the filesystem the user programs see.
//!
//! The kernel owns exactly one filesystem. It is created when the kernel
//! starts, it holds the programs of the machine, and every system call that
//! touches a file goes through it.
//!
//! # The medium
//!
//! The filesystem lives on a disk when the machine has one: the kernel
//! looks for a block device of the PCI bus that is not the disk the machine
//! boots from, and it mounts the filesystem on it. A machine without such a
//! disk keeps its filesystem in memory, and forgets everything when it
//! stops.
//!
//! # The lock
//!
//! The filesystem is behind a lock, because a system call may arrive while
//! the kernel is doing something else with the filesystem. The lock is not
//! recursive: a program that runs while the filesystem is locked would
//! deadlock, so the kernel always releases the lock before a program runs.

use alloc::boxed::Box;
use spin::Mutex;

use crate::block::{BLOCK_SIZE, BlockDevice};
use crate::fs::{FileSystem, FsError, MemFs, Permissions};
use crate::vmm::mapper::Mapper;

/// The filesystem of the machine.
static FILESYSTEM: Mutex<Option<MemFs>> = Mutex::new(None);

/// The directories the filesystem of the machine holds.
pub const DIRECTORIES: [&str; 4] = ["/bin", "/dev", "/etc", "/tmp"];

/// The signature of a partition table at the start of a disk.
const PARTITION_TABLE: &[u8; 8] = b"EFI PART";

/// Build the filesystem of the machine and mount it.
///
/// The function looks for a disk the machine can keep its files on. The
/// directories of the system and the programs of the initial userland are
/// installed the first time the machine starts on a disk; a disk that
/// already holds a filesystem keeps what it has.
pub fn initialize(mapper: &mut Mapper) -> Result<(), FsError> {
    let mut on_disk = false;
    let mut fs = match open_data_disk(mapper) {
        Ok(Some(fs)) => {
            on_disk = true;
            fs
        }
        Ok(None) => MemFs::new(),
        Err(_) => {
            // A disk that cannot be opened is left alone: the machine
            // starts with a filesystem in memory.
            unsafe {
                crate::serial::write_str(
                    "Elvaris kernel: storage: the disk could not be opened\r\n",
                );
            }
            MemFs::new()
        }
    };

    if let Err(err) = fs.mount() {
        unsafe {
            crate::serial::write_str("Elvaris kernel: storage: the disk could not be mounted: ");
            crate::serial::write_fmt(core::format_args!("{:?}\r\n", err));
        }
        let mut memory = MemFs::new();
        memory.mount()?;
        fs = memory;
        on_disk = false;
    }

    // The machine installs the system the first time it sees an empty
    // filesystem.
    let empty = fs.read_dir("/").map(|entries| entries.is_empty()).unwrap_or(false);
    if empty {
        for directory in DIRECTORIES {
            fs.create_dir(directory, Permissions::directory())?;
        }
        crate::user::programs::install(&mut fs)?;
        fs.sync()?;
        unsafe {
            crate::serial::write_str("Elvaris kernel: storage: the system was installed\r\n");
        }
    }

    unsafe {
        if on_disk {
            crate::serial::write_str("Elvaris kernel: storage: the filesystem is on the disk\r\n");
        } else {
            crate::serial::write_str("Elvaris kernel: storage: the filesystem is in memory\r\n");
        }
    }

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

/// Write the filesystem of the machine back into its device.
///
/// The kernel calls the function before the machine stops, so the files of
/// the machine survive a restart.
pub fn sync() -> Result<(), FsError> {
    match with_filesystem(|fs| fs.sync()) {
        Ok(result) => result,
        Err(_) => Err(FsError::NotMounted),
    }
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

/// The disk the machine can keep its files on.
///
/// The function looks for a block device of the PCI bus. The disk the
/// machine boots from holds the partition table of the machine, so it is
/// not a data disk, and it is skipped.
fn open_data_disk(mapper: &mut Mapper) -> Result<Option<MemFs>, FsError> {
    for device_number in 0..8u8 {
        for function in 0..2u8 {
            let location = crate::pci::Location {
                bus: 0,
                device: device_number,
                function,
            };
            let candidate = crate::pci::Device { location };
            unsafe {
                crate::serial::write_fmt(core::format_args!(
                    "Elvaris kernel: storage: probe {}:{} vendor={:04x} device={:04x}\r\n",
                    device_number,
                    function,
                    candidate.vendor_id(),
                    candidate.device_id()
                ));
            }
        }
    }
    let devices = crate::pci::find_block_devices();
    unsafe {
        crate::serial::write_fmt(core::format_args!(
            "Elvaris kernel: storage: block devices on the bus: {}\r\n",
            devices.len()
        ));
    }
    for device in &devices {
        let capabilities = device.virtio_capabilities();
        unsafe {
            crate::serial::write_fmt(core::format_args!(
                "Elvaris kernel: storage: block device {}:{} capabilities={}\r\n",
                device.location.bus,
                device.location.device,
                capabilities.len()
            ));
        }
    }
    for (slot, device) in devices.into_iter().enumerate() {
        let block_device = match crate::block::virtio::VirtioBlockDevice::open(&device, mapper, slot) {
            Ok(device) => device,
            Err(error) => {
                unsafe {
                    crate::serial::write_fmt(core::format_args!(
                        "Elvaris kernel: storage: the block device could not be opened: {:?}\r\n",
                        error
                    ));
                }
                continue;
            }
        };
        if holds_partition_table(&block_device) {
            continue;
        }
        return Ok(Some(MemFs::with_device(Box::new(block_device))));
    }
    Ok(None)
}

/// Whether a disk holds a partition table.
///
/// A disk that holds one is the disk the machine boots from.
fn holds_partition_table(device: &crate::block::virtio::VirtioBlockDevice) -> bool {
    let mut block = [0u8; BLOCK_SIZE];
    if device.read_block(0, &mut block).is_err() {
        return false;
    }
    &block[0x200..0x206] == PARTITION_TABLE
}
