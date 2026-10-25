//! The filesystem interface.
//!
//! The filesystem is the layer between the kernel and the storage. It knows
//! about files, directories, metadata and permissions; it knows nothing about
//! the shell, about the program loader, about the format of a program image
//! and about the emulator the kernel runs in.
//!
//! # The interface
//!
//! | Operation | Purpose |
//! |-----------|---------|
//! | [`FileSystem::mount`] | Make the filesystem usable. |
//! | [`FileSystem::unmount`] | Make the filesystem unusable again. |
//! | [`FileSystem::create_file`] | Create an empty file. |
//! | [`FileSystem::create_dir`] | Create a directory. |
//! | [`FileSystem::open`] | Open a file and obtain a handle. |
//! | [`FileSystem::close`] | Release a handle. |
//! | [`FileSystem::read`] | Read from an open file. |
//! | [`FileSystem::write`] | Write into an open file. |
//! | [`FileSystem::seek`] | Move the position of an open file. |
//! | [`FileSystem::delete`] | Remove a file or an empty directory. |
//! | [`FileSystem::read_dir`] | List the entries of a directory. |
//! | [`FileSystem::stat`] | Read the metadata of an object. |
//! | [`FileSystem::set_permissions`] | Change the permissions of an object. |
//!
//! # Objects
//!
//! A filesystem holds two kinds of objects:
//!
//! - **Files** hold a sequence of bytes. A file has a size, and a position
//!   that moves when it is read or written.
//! - **Directories** hold named entries. Every entry is a file or another
//!   directory. A directory has no content of its own.
//!
//! # Handles
//!
//! A file is not used through its path while it is open: it is used through
//! a handle. A handle refers to exactly one open file, carries its own
//! position, and belongs to the process that opened it. The kernel keeps the
//! handle table of a filesystem, so a handle stays valid until it is closed
//! or until the file it refers to is deleted.
//!
//! # Paths
//!
//! The rules for paths are described in [`path`]. In short:
//!
//! - `/` separates the components of a path and is the root itself,
//! - `.` is the current directory and `..` is the parent directory,
//! - repeated separators are collapsed,
//! - an empty path is invalid,
//! - a relative path is resolved against the root, because the kernel has no
//!   per-process current directory yet.
//!
//! # Errors
//!
//! Every operation that can fail reports why with an [`FsError`]. The errors
//! never describe an internal state: they are the answer to the question
//! "what did the caller ask for that the filesystem cannot do?".

use alloc::string::String;
use alloc::vec::Vec;

pub mod memfs;
pub mod path;

#[cfg(test)]
mod tests;

pub use memfs::MemFs;
pub use path::{Path, PathError};

/// The errors a filesystem reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    /// The object does not exist.
    NotFound,
    /// The path is not a valid path.
    InvalidPath,
    /// The caller is not allowed to do what it asked for.
    PermissionDenied,
    /// The medium failed.
    IoError,
    /// An object with that name already exists.
    AlreadyExists,
    /// The path names a directory, but a file was expected.
    NotADirectory,
    /// The path names a file, but a directory was expected.
    IsADirectory,
    /// The directory is not empty.
    NotEmpty,
    /// The filesystem has no free handle left.
    TooManyOpenFiles,
    /// The arguments of the operation are not usable.
    InvalidArgument,
    /// The filesystem is not mounted.
    NotMounted,
    /// The operation is not supported by this filesystem.
    Unsupported,
}

impl FsError {
    /// A short description of the error, used in the serial diagnostics.
    pub const fn description(&self) -> &'static str {
        match self {
            FsError::NotFound => "the object does not exist",
            FsError::InvalidPath => "the path is not valid",
            FsError::PermissionDenied => "the operation is not permitted",
            FsError::IoError => "the medium failed",
            FsError::AlreadyExists => "the object already exists",
            FsError::NotADirectory => "the path does not name a directory",
            FsError::IsADirectory => "the path names a directory",
            FsError::NotEmpty => "the directory is not empty",
            FsError::TooManyOpenFiles => "there is no free file handle left",
            FsError::InvalidArgument => "the arguments are not valid",
            FsError::NotMounted => "the filesystem is not mounted",
            FsError::Unsupported => "the operation is not supported",
        }
    }
}

impl From<PathError> for FsError {
    fn from(err: PathError) -> Self {
        match err {
            PathError::TooLong => FsError::InvalidPath,
            PathError::Empty => FsError::InvalidPath,
            PathError::InvalidCharacter => FsError::InvalidPath,
            PathError::InvalidComponent => FsError::InvalidPath,
        }
    }
}

/// The kind of an object in the filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// A file: a sequence of bytes.
    File,
    /// A directory: a list of named entries.
    Directory,
}

impl FileType {
    /// A short name of the kind, used in the diagnostics.
    pub const fn name(&self) -> &'static str {
        match self {
            FileType::File => "file",
            FileType::Directory => "directory",
        }
    }
}

/// The owner of an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// The object belongs to the system.
    System,
    /// The object belongs to the user with that identifier.
    User(u64),
    /// The object belongs to specific user and group
    UserGroup { uid: u64, gid: u64 },
}

impl Owner {
    /// Whether the owner allows the given identity to use the object.
    pub const fn matches(&self, identity: &Identity) -> bool {
        match self {
            Owner::System => true,
            Owner::User(uid) => *uid == identity.uid,
            Owner::UserGroup { uid, .. } => *uid == identity.uid,
        }
    }
}

/// The identity of a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    /// The identifier of the user.
    pub uid: u64,
    /// The identifier of the group.
    pub gid: u64,
}

impl Identity {
    /// The identity of the kernel itself.
    pub const fn system() -> Self {
        Identity { uid: 0, gid: 0 }
    }
}

/// What a caller wants to do with an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Read the content of the object.
    Read,
    /// Change the content of the object.
    Write,
    /// Run the object as a program.
    Execute,
}

/// The permissions of an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    /// The owner of the object.
    pub owner: Owner,
    /// The object may be read.
    pub read: bool,
    /// The object may be written.
    pub write: bool,
    /// The object may be executed.
    pub execute: bool,
}

impl Permissions {
    /// The permissions of a file that everybody may read.
    pub const fn public_read() -> Self {
        Permissions {
            owner: Owner::System,
            read: true,
            write: false,
            execute: false,
        }
    }

    /// The permissions of a file that everybody may read and execute.
    pub const fn public_execute() -> Self {
        Permissions {
            owner: Owner::System,
            read: true,
            write: false,
            execute: true,
        }
    }

    /// The permissions of a program everybody may read and run.
    pub const fn program() -> Self {
        Permissions {
            owner: Owner::System,
            read: true,
            write: true,
            execute: true,
        }
    }

    /// The permissions of a file that only the system may change.
    pub const fn system_write() -> Self {
        Permissions {
            owner: Owner::System,
            read: true,
            write: true,
            execute: false,
        }
    }

    /// The permissions of a file that only the owner may change.
    pub const fn owner_only() -> Self {
        Permissions {
            owner: Owner::User(1000),
            read: true,
            write: true,
            execute: false,
        }
    }

    /// The permissions of a root-owned system file.
    pub const fn root_only() -> Self {
        Permissions {
            owner: Owner::User(0),
            read: true,
            write: true,
            execute: false,
        }
    }

    /// Whether the identity may perform the access.
    pub const fn allows(&self, identity: &Identity, access: Access) -> bool {
        if !self.owner.matches(identity) {
            return false;
        }
        match access {
            Access::Read => self.read,
            Access::Write => self.write,
            Access::Execute => self.execute,
        }
    }
}

/// The metadata of an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMetadata {
    /// The kind of the object.
    pub file_type: FileType,
    /// The size of the object in bytes. A directory has no size.
    pub size: u64,
    /// The owner of the object.
    pub owner: Owner,
    /// The permissions of the object.
    pub permissions: Permissions,
}

/// An entry of a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    /// The name of the entry, without any path.
    pub name: String,
    /// The kind of the entry.
    pub file_type: FileType,
}

/// The flags a file is opened with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenFlags {
    /// The file may be read.
    pub read: bool,
    /// The file may be written.
    pub write: bool,
    /// The file is created when it does not exist.
    pub create: bool,
    /// The content of the file is discarded when it is opened.
    pub truncate: bool,
    /// The file may be executed.
    pub execute: bool,
}

impl OpenFlags {
    /// Open the file for reading.
    pub const fn read() -> Self {
        OpenFlags {
            read: true,
            write: false,
            create: false,
            truncate: false,
            execute: false,
        }
    }

    /// Open the file for writing.
    pub const fn write() -> Self {
        OpenFlags {
            read: false,
            write: true,
            create: false,
            truncate: false,
            execute: false,
        }
    }

    /// Open the file for reading and for writing.
    pub const fn read_write() -> Self {
        OpenFlags {
            read: true,
            write: true,
            create: false,
            truncate: false,
            execute: false,
        }
    }

    /// Open the file for reading and for writing, creating it when it is
    /// missing.
    pub const fn read_write_create() -> Self {
        OpenFlags {
            read: true,
            write: true,
            create: true,
            truncate: false,
            execute: false,
        }
    }

    /// Open the file for writing, creating it when it is missing.
    pub const fn create() -> Self {
        OpenFlags {
            read: false,
            write: true,
            create: true,
            truncate: false,
            execute: false,
        }
    }

    /// Open the file for writing and discard its content.
    pub const fn truncate() -> Self {
        OpenFlags {
            read: false,
            write: true,
            create: true,
            truncate: true,
            execute: false,
        }
    }

    /// The access the flags ask for, if any.
    pub const fn access(&self) -> Option<Access> {
        if self.write {
            Some(Access::Write)
        } else if self.read {
            Some(Access::Read)
        } else {
            None
        }
    }
}

/// A handle of an open file.
///
/// The handle is a small, copyable value: the kernel hands it to the user
/// programs as a number, and the filesystem uses it as the index of its
/// handle table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileHandle(pub u32);

impl FileHandle {
    /// The raw number of the handle.
    pub const fn raw(&self) -> u32 {
        self.0
    }
}

/// The filesystem the kernel works with.
///
/// The trait is the contract between the kernel and every filesystem. It
/// says nothing about how the filesystem stores its data: the in-memory
/// filesystem of this milestone keeps the tree in the kernel heap and the
/// content in a block device.
pub trait FileSystem {
    /// Make the filesystem usable.
    ///
    /// The operation is refused with [`FsError::AlreadyExists`] when the
    /// filesystem is mounted already.
    fn mount(&mut self) -> Result<(), FsError>;

    /// Make the filesystem unusable again.
    ///
    /// The operation writes the changes back to the medium before it
    /// releases the filesystem.
    fn unmount(&mut self) -> Result<(), FsError>;

    /// Whether the filesystem is mounted.
    fn is_mounted(&self) -> bool;

    /// Create an empty file.
    ///
    /// The parent directory has to exist. The operation is refused with
    /// [`FsError::AlreadyExists`] when the name is taken.
    fn create_file(&mut self, path: &str, permissions: Permissions) -> Result<(), FsError>;

    /// Create a directory.
    ///
    /// The parent directory has to exist. The operation is refused with
    /// [`FsError::AlreadyExists`] when the name is taken.
    fn create_dir(&mut self, path: &str, permissions: Permissions) -> Result<(), FsError>;

    /// Open an object and obtain a handle.
    ///
    /// A file that does not exist is created when the flags ask for it.
    fn open(&mut self, path: &str, flags: OpenFlags) -> Result<FileHandle, FsError>;

    /// Release a handle.
    fn close(&mut self, handle: FileHandle) -> Result<(), FsError>;

    /// Obtain a second handle of an open file.
    ///
    /// The second handle has its own position: it starts at the beginning of
    /// the file.
    fn duplicate(&mut self, handle: FileHandle) -> Result<FileHandle, FsError>;

    /// Read from an open file into `buffer`.
    ///
    /// The function returns the number of bytes it read. Reading at the end
    /// of the file returns zero.
    fn read(&mut self, handle: FileHandle, buffer: &mut [u8]) -> Result<usize, FsError>;

    /// Write into an open file.
    ///
    /// The function returns the number of bytes it wrote. Writing at the end
    /// of the file extends it.
    fn write(&mut self, handle: FileHandle, buffer: &[u8]) -> Result<usize, FsError>;

    /// Move the position of an open file.
    ///
    /// A position beyond the end of the file is allowed: the next write
    /// extends the file up to that position.
    fn seek(&mut self, handle: FileHandle, position: u64) -> Result<u64, FsError>;

    /// Remove a file or an empty directory.
    fn delete(&mut self, path: &str) -> Result<(), FsError>;

    /// List the entries of a directory.
    fn read_dir(&mut self, path: &str) -> Result<Vec<DirEntry>, FsError>;

    /// Read the metadata of an object.
    fn stat(&mut self, path: &str) -> Result<FileMetadata, FsError>;

    /// Change the permissions of an object.
    fn set_permissions(&mut self, path: &str, permissions: Permissions) -> Result<(), FsError>;

    /// Whether an object exists.
    fn exists(&mut self, path: &str) -> bool;

    /// The number of handles that are open.
    fn open_handle_count(&self) -> usize;
}
