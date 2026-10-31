//! The in-memory filesystem.
//!
//! The filesystem keeps its directory tree in the kernel heap and its whole
//! content in a block device. It is the first filesystem of Elvaris, and it
//! is the one the userland of this milestone runs on.
//!
//! # Layering
//!
//! ```text
//! Kernel (syscalls, loader)
//!     |
//! Filesystem interface (fs)
//!     |
//! In-memory filesystem (fs::memfs)
//!     |
//! Block device (block)
//!     |
//! Memory
//! ```
//!
//! The filesystem never talks to the memory directly: it writes its blocks
//! into the device and reads them back from it. The medium is therefore
//! replaceable, and the format of the filesystem does not depend on the
//! device underneath it.
//!
//! # The image
//!
//! The content of the filesystem is serialized into an *image*: a byte blob
//! that starts with a header and holds the objects of the tree. The image is
//! what the device holds.
//!
//! ```text
//!  magic        8 bytes   "ELVFS1\0\0"
//!  version      4 bytes   1
//!  entry_count  4 bytes   the number of objects
//!  entries                   one record per object
//! ```
//!
//! Every entry holds the path of the object, its kind, its permissions, its
//! owner and — for a file — its content. The entries are sorted by path, so
//! that the image of the same tree is always the same.
//!
//! # Mounting
//!
//! The kernel writes the image of the initial filesystem into the device and
//! then mounts the filesystem. Mounting reads the image back and builds the
//! tree. The changes are written back into the device when the filesystem is
//! unmounted or when it is synchronized explicitly.
//!
//! # Limitations
//!
//! - The tree lives in the kernel heap. The heap is not grown on demand, so
//!   a filesystem with many objects can exhaust it.
//! - A file that is open can be deleted. The handles of a deleted file
//!   report [`FsError::NotFound`] when they are used.
//! - There is no symbolic link, no hard link, no device file and no named
//!   pipe.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::block::{BLOCK_SIZE, BlockDevice, BlockError, MemoryBlockDevice};
use crate::fs::path::Path;
use crate::fs::{
    Access, DirEntry, FileHandle, FileMetadata, FileSystem, FileType, FsError, Identity, OpenFlags,
    Owner, Permissions,
};

/// The magic that starts every image of this filesystem.
pub const IMAGE_MAGIC: [u8; 8] = *b"ELVFS1\0\0";

/// The version of the image format this module reads and writes.
pub const IMAGE_VERSION: u32 = 1;

/// The kind of an object in the image.
const KIND_FILE: u8 = 0;
const KIND_DIRECTORY: u8 = 1;

/// The owner of an object in the image.
const OWNER_SYSTEM: u8 = 0;
const OWNER_USER: u8 = 1;

/// The number of blocks the device of a new filesystem holds.
pub const DEFAULT_BLOCKS: usize = 64;

/// The number of blocks the image of the filesystem may use.
///
/// The image of the filesystem is read into the memory of the kernel when
/// the filesystem is mounted, so the image has to be small enough to fit
/// into the heap. A filesystem that is bigger than this does not fit.
pub const MAX_IMAGE_BLOCKS: usize = 128;

/// The maximum number of handles that are open at the same time.
pub const MAX_OPEN_HANDLES: usize = 32;

/// A node of the directory tree.
#[derive(Debug, Clone)]
enum Node {
    /// A directory: named entries.
    Directory {
        entries: Vec<(String, Node)>,
        permissions: Permissions,
    },
    /// A file: a sequence of bytes.
    File {
        data: Vec<u8>,
        permissions: Permissions,
    },
}

impl Node {
    /// The kind of the node.
    fn file_type(&self) -> FileType {
        match self {
            Node::Directory { .. } => FileType::Directory,
            Node::File { .. } => FileType::File,
        }
    }

    /// The permissions of the node.
    fn permissions(&self) -> Permissions {
        match self {
            Node::Directory { permissions, .. } => *permissions,
            Node::File { permissions, .. } => *permissions,
        }
    }

    /// The size of the node. A directory has no size.
    fn size(&self) -> u64 {
        match self {
            Node::Directory { .. } => 0,
            Node::File { data, .. } => data.len() as u64,
        }
    }

    /// The entry with that name.
    fn entry(&self, name: &str) -> Option<&Node> {
        match self {
            Node::Directory { entries, .. } => entries
                .iter()
                .find(|(entry_name, _)| entry_name == name)
                .map(|(_, node)| node),
            Node::File { .. } => None,
        }
    }

    /// The entry with that name, for writing.
    fn entry_mut(&mut self, name: &str) -> Option<&mut Node> {
        match self {
            Node::Directory { entries, .. } => entries
                .iter_mut()
                .find(|(entry_name, _)| entry_name == name)
                .map(|(_, node)| node),
            Node::File { .. } => None,
        }
    }

    /// Add an entry to a directory.
    fn insert(&mut self, name: &str, node: Node) -> Result<(), FsError> {
        match self {
            Node::Directory { entries, .. } => {
                if entries.iter().any(|(entry_name, _)| entry_name == name) {
                    return Err(FsError::AlreadyExists);
                }
                entries.push((name.to_string(), node));
                Ok(())
            }
            Node::File { .. } => Err(FsError::NotADirectory),
        }
    }

    /// Remove an entry from a directory.
    fn remove(&mut self, name: &str) -> Option<Node> {
        match self {
            Node::Directory { entries, .. } => {
                let index = entries
                    .iter()
                    .position(|(entry_name, _)| entry_name == name)?;
                Some(entries.remove(index).1)
            }
            Node::File { .. } => None,
        }
    }
}

/// A handle of an open file.
#[derive(Debug, Clone)]
struct Handle {
    path: Path,
    position: u64,
    flags: OpenFlags,
}

/// The in-memory filesystem.
pub struct MemFs {
    device: Box<dyn BlockDevice>,
    root: Node,
    handles: Vec<Option<Handle>>,
    mounted: bool,
    identity: Identity,
}

impl MemFs {
    /// Create an empty filesystem with an empty device.
    pub fn new() -> Self {
        Self::with_blocks(DEFAULT_BLOCKS)
    }

    /// Create an empty filesystem with a device of `blocks` blocks.
    pub fn with_blocks(blocks: usize) -> Self {
        Self::with_device(Box::new(MemoryBlockDevice::new(blocks)))
    }

    /// Create an empty filesystem on a device.
    ///
    /// The device is the medium of the filesystem: a device in memory, or a
    /// disk in the host. The filesystem writes its image into the device,
    /// and reads it back from it when it is mounted.
    pub fn with_device(device: Box<dyn BlockDevice>) -> Self {
        MemFs {
            device,
            root: Node::Directory {
                entries: Vec::new(),
                permissions: Permissions::directory(),
            },
            handles: vec![None; MAX_OPEN_HANDLES],
            mounted: false,
            identity: Identity::system(),
        }
    }

    /// The identity the filesystem acts with.
    pub fn identity(&self) -> Identity {
        self.identity
    }

    /// Change the identity the filesystem acts with.
    pub fn set_identity(&mut self, identity: Identity) {
        self.identity = identity;
    }

    /// The device the filesystem keeps its content in.
    pub fn device(&self) -> &dyn BlockDevice {
        self.device.as_ref()
    }

    /// The device the filesystem keeps its content in, for writing.
    pub fn device_mut(&mut self) -> &mut dyn BlockDevice {
        self.device.as_mut()
    }

    /// Write an image into the device of the filesystem.
    ///
    /// The image is the content the filesystem holds when it is mounted. The
    /// function is used to install the initial filesystem.
    pub fn install_image(&mut self, image: &[u8]) -> Result<(), FsError> {
        let needed = image.len().div_ceil(BLOCK_SIZE);
        if needed > self.device.block_count() {
            return Err(FsError::IoError);
        }
        let mut block = [0u8; BLOCK_SIZE];
        for index in 0..needed {
            let start = index * BLOCK_SIZE;
            let end = core::cmp::min(start + BLOCK_SIZE, image.len());
            block[..end - start].copy_from_slice(&image[start..end]);
            for byte in block.iter_mut().skip(end - start) {
                *byte = 0;
            }
            self.device
                .write_block(index, &block)
                .map_err(map_block_error)?;
        }
        Ok(())
    }

    /// The image of the tree, ready to be written into a device.
    pub fn image(&self) -> Vec<u8> {
        let mut entries: Vec<(String, Node)> = Vec::new();
        self.collect_entries(&self.root, "", &mut entries);
        entries.sort_by(|left, right| left.0.cmp(&right.0));

        let mut image: Vec<u8> = Vec::new();
        image.extend_from_slice(&IMAGE_MAGIC);
        image.extend_from_slice(&IMAGE_VERSION.to_le_bytes());
        image.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        for (path, node) in &entries {
            image.extend_from_slice(&(path.len() as u32).to_le_bytes());
            image.extend_from_slice(path.as_bytes());
            match node {
                Node::Directory { permissions, .. } => {
                    image.push(KIND_DIRECTORY);
                    write_permissions(&mut image, *permissions);
                }
                Node::File { data, permissions } => {
                    image.push(KIND_FILE);
                    write_permissions(&mut image, *permissions);
                    image.extend_from_slice(&(data.len() as u64).to_le_bytes());
                    image.extend_from_slice(data);
                }
            }
        }
        image
    }

    /// Write the tree back into the device.
    pub fn sync(&mut self) -> Result<(), FsError> {
        let image = self.image();
        self.install_image(&image)
    }

    /// Collect the objects of the tree, with their paths.
    fn collect_entries(&self, node: &Node, prefix: &str, entries: &mut Vec<(String, Node)>) {
        if let Node::Directory {
            entries: children, ..
        } = node
        {
            for (name, child) in children {
                let path = format!("{prefix}/{name}");
                entries.push((path.clone(), child.clone()));
                self.collect_entries(child, &path, entries);
            }
        }
    }

    /// The node the path names.
    fn find(&self, path: &Path) -> Option<&Node> {
        let mut node = &self.root;
        for component in path.components() {
            node = node.entry(component)?;
        }
        Some(node)
    }

    /// The node the path names, for writing.
    fn find_mut(&mut self, path: &Path) -> Option<&mut Node> {
        let mut node = &mut self.root;
        for component in path.components() {
            node = node.entry_mut(component)?;
        }
        Some(node)
    }

    /// The directory that holds the object the path names.
    fn find_parent_mut(&mut self, path: &Path) -> Option<&mut Node> {
        let parent = path.parent()?;
        self.find_mut(&parent)
    }

    /// The handle with that number.
    fn handle(&self, handle: FileHandle) -> Result<&Handle, FsError> {
        self.handles
            .get(handle.0 as usize)
            .and_then(|slot| slot.as_ref())
            .ok_or(FsError::InvalidArgument)
    }

    /// The handle with that number, for writing.
    fn handle_mut(&mut self, handle: FileHandle) -> Result<&mut Handle, FsError> {
        self.handles
            .get_mut(handle.0 as usize)
            .and_then(|slot| slot.as_mut())
            .ok_or(FsError::InvalidArgument)
    }

    /// A free slot of the handle table.
    fn free_handle(&mut self) -> Result<FileHandle, FsError> {
        for (index, slot) in self.handles.iter().enumerate() {
            if slot.is_none() {
                return Ok(FileHandle(index as u32));
            }
        }
        Err(FsError::TooManyOpenFiles)
    }

    /// Read the tree out of the image of the device.
    ///
    /// A device that holds no image at all describes an empty filesystem:
    /// the mount succeeds and the root has no entries.
    fn load_image(&mut self) -> Result<(), FsError> {
        let bytes = self.read_image()?;
        if bytes.len() < IMAGE_MAGIC.len() + 8 || bytes[..IMAGE_MAGIC.len()] == [0u8; 8] {
            // The device is empty: the filesystem starts with an empty root.
            self.root = Node::Directory {
                entries: Vec::new(),
                permissions: Permissions::directory(),
            };
            return Ok(());
        }
        if bytes[..IMAGE_MAGIC.len()] != IMAGE_MAGIC {
            return Err(FsError::IoError);
        }
        let mut offset = IMAGE_MAGIC.len();
        let version = read_u32(&bytes, &mut offset)?;
        if version != IMAGE_VERSION {
            return Err(FsError::Unsupported);
        }
        let entry_count = read_u32(&bytes, &mut offset)?;

        let mut root = Node::Directory {
            entries: Vec::new(),
            permissions: Permissions::directory(),
        };

        for _ in 0..entry_count {
            let path_length = read_u32(&bytes, &mut offset)? as usize;
            if offset + path_length > bytes.len() {
                return Err(FsError::IoError);
            }
            let path_text = core::str::from_utf8(&bytes[offset..offset + path_length])
                .map_err(|_| FsError::InvalidPath)?
                .to_string();
            offset += path_length;

            let kind = *bytes.get(offset).ok_or(FsError::IoError)?;
            offset += 1;
            let permissions = read_permissions(&bytes, &mut offset)?;

            let node = match kind {
                KIND_DIRECTORY => Node::Directory {
                    entries: Vec::new(),
                    permissions,
                },
                KIND_FILE => {
                    let size = read_u64(&bytes, &mut offset)? as usize;
                    if offset + size > bytes.len() {
                        return Err(FsError::IoError);
                    }
                    let data = bytes[offset..offset + size].to_vec();
                    offset += size;
                    Node::File { data, permissions }
                }
                _ => return Err(FsError::IoError),
            };

            insert_into_tree(&mut root, &path_text, node)?;
        }

        self.root = root;
        Ok(())
    }

    /// Read the image of the filesystem out of the device.
    ///
    /// The image is read into the memory of the kernel, so only the first
    /// blocks of the device are read: a device that is much bigger than the
    /// image holds nothing else.
    fn read_image(&self) -> Result<Vec<u8>, FsError> {
        let count = self.device.block_count().min(MAX_IMAGE_BLOCKS);
        let mut bytes = alloc::vec![0u8; count * BLOCK_SIZE];
        self.device
            .read_blocks(0, &mut bytes)
            .map_err(map_block_error)?;
        Ok(bytes)
    }
}

impl Default for MemFs {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystem for MemFs {
    fn mount(&mut self) -> Result<(), FsError> {
        if self.mounted {
            return Err(FsError::AlreadyExists);
        }
        self.load_image()?;
        self.mounted = true;
        Ok(())
    }

    fn unmount(&mut self) -> Result<(), FsError> {
        if !self.mounted {
            return Err(FsError::NotMounted);
        }
        self.sync()?;
        for slot in self.handles.iter_mut() {
            *slot = None;
        }
        self.mounted = false;
        Ok(())
    }

    fn is_mounted(&self) -> bool {
        self.mounted
    }

    fn create_file(&mut self, path: &str, permissions: Permissions) -> Result<(), FsError> {
        if !self.mounted {
            return Err(FsError::NotMounted);
        }
        let path = Path::parse(path)?;
        if path.is_root() {
            return Err(FsError::InvalidArgument);
        }
        let name = path.file_name().ok_or(FsError::InvalidPath)?;
        let identity = self.identity;
        let parent = self.find_parent_mut(&path).ok_or(FsError::NotFound)?;
        match parent {
            Node::Directory {
                permissions: parent_permissions,
                ..
            } => {
                if !parent_permissions.allows(&identity, Access::Write) {
                    return Err(FsError::PermissionDenied);
                }
            }
            Node::File { .. } => return Err(FsError::NotADirectory),
        }
        parent.insert(
            name,
            Node::File {
                data: Vec::new(),
                permissions,
            },
        )
    }

    fn create_dir(&mut self, path: &str, permissions: Permissions) -> Result<(), FsError> {
        if !self.mounted {
            return Err(FsError::NotMounted);
        }
        let path = Path::parse(path)?;
        if path.is_root() {
            return Err(FsError::AlreadyExists);
        }
        let name = path.file_name().ok_or(FsError::InvalidPath)?;
        let identity = self.identity;
        let parent = self.find_parent_mut(&path).ok_or(FsError::NotFound)?;
        match parent {
            Node::Directory {
                permissions: parent_permissions,
                ..
            } => {
                if !parent_permissions.allows(&identity, Access::Write) {
                    return Err(FsError::PermissionDenied);
                }
            }
            Node::File { .. } => return Err(FsError::NotADirectory),
        }
        parent.insert(
            name,
            Node::Directory {
                entries: Vec::new(),
                permissions,
            },
        )
    }

    fn open(&mut self, path: &str, flags: OpenFlags) -> Result<FileHandle, FsError> {
        if !self.mounted {
            return Err(FsError::NotMounted);
        }
        let path = Path::parse(path)?;
        if path.is_root() {
            return Err(FsError::IsADirectory);
        }

        let exists = self.find(&path).is_some();
        if !exists {
            if !flags.create {
                return Err(FsError::NotFound);
            }
            // The parent has to exist before the file can be created.
            let uid = self.identity.uid;
            let parent = self.find_parent_mut(&path).ok_or(FsError::NotFound)?;
            let name = path.file_name().ok_or(FsError::InvalidPath)?;
            parent.insert(
                name,
                Node::File {
                    data: Vec::new(),
                    permissions: Permissions {
                        owner: Owner::User(uid),
                        read: true,
                        write: true,
                        execute: flags.execute,
                    },
                },
            )?;
        }

        let node = self.find(&path).ok_or(FsError::NotFound)?;
        if node.file_type() == FileType::Directory {
            return Err(FsError::IsADirectory);
        }
        let permissions = node.permissions();
        if flags.read && !permissions.allows(&self.identity, Access::Read) {
            return Err(FsError::PermissionDenied);
        }
        if flags.write && !permissions.allows(&self.identity, Access::Write) {
            return Err(FsError::PermissionDenied);
        }
        if flags.execute && !permissions.allows(&self.identity, Access::Execute) {
            return Err(FsError::PermissionDenied);
        }

        if flags.truncate {
            if let Some(Node::File { data, .. }) = self.find_mut(&path) {
                data.clear();
            }
        }

        let slot = self.free_handle()?;
        self.handles[slot.0 as usize] = Some(Handle {
            path,
            position: 0,
            flags,
        });
        Ok(slot)
    }

    fn close(&mut self, handle: FileHandle) -> Result<(), FsError> {
        if !self.mounted {
            return Err(FsError::NotMounted);
        }
        let slot = self
            .handles
            .get_mut(handle.0 as usize)
            .ok_or(FsError::InvalidArgument)?;
        if slot.is_none() {
            return Err(FsError::InvalidArgument);
        }
        *slot = None;
        Ok(())
    }

    fn duplicate(&mut self, handle: FileHandle) -> Result<FileHandle, FsError> {
        if !self.mounted {
            return Err(FsError::NotMounted);
        }
        let source = self.handle(handle)?.clone();
        let slot = self.free_handle()?;
        self.handles[slot.0 as usize] = Some(Handle {
            path: source.path,
            position: 0,
            flags: source.flags,
        });
        Ok(slot)
    }

    fn read(&mut self, handle: FileHandle, buffer: &mut [u8]) -> Result<usize, FsError> {
        if !self.mounted {
            return Err(FsError::NotMounted);
        }
        let (path, position) = {
            let entry = self.handle(handle)?;
            (entry.path.clone(), entry.position)
        };
        if !self.handle(handle)?.flags.read {
            return Err(FsError::PermissionDenied);
        }
        let node = self.find(&path).ok_or(FsError::NotFound)?;
        let Node::File { data, permissions } = node else {
            return Err(FsError::IsADirectory);
        };
        if !permissions.allows(&self.identity, Access::Read) {
            return Err(FsError::PermissionDenied);
        }
        let start = core::cmp::min(position as usize, data.len());
        let end = core::cmp::min(start + buffer.len(), data.len());
        let count = end - start;
        buffer[..count].copy_from_slice(&data[start..end]);
        self.handle_mut(handle)?.position = position + count as u64;
        Ok(count)
    }

    fn write(&mut self, handle: FileHandle, buffer: &[u8]) -> Result<usize, FsError> {
        if !self.mounted {
            return Err(FsError::NotMounted);
        }
        let (path, position) = {
            let entry = self.handle(handle)?;
            (entry.path.clone(), entry.position)
        };
        if !self.handle(handle)?.flags.write {
            return Err(FsError::PermissionDenied);
        }
        let identity = self.identity;
        let node = self.find_mut(&path).ok_or(FsError::NotFound)?;
        let Node::File { data, permissions } = node else {
            return Err(FsError::IsADirectory);
        };
        if !permissions.allows(&identity, Access::Write) {
            return Err(FsError::PermissionDenied);
        }
        let start = position as usize;
        if data.len() < start + buffer.len() {
            data.resize(start + buffer.len(), 0);
        }
        data[start..start + buffer.len()].copy_from_slice(buffer);
        self.handle_mut(handle)?.position = position + buffer.len() as u64;
        Ok(buffer.len())
    }

    fn seek(&mut self, handle: FileHandle, position: u64) -> Result<u64, FsError> {
        if !self.mounted {
            return Err(FsError::NotMounted);
        }
        self.handle_mut(handle)?.position = position;
        Ok(position)
    }

    fn delete(&mut self, path: &str) -> Result<(), FsError> {
        if !self.mounted {
            return Err(FsError::NotMounted);
        }
        let path = Path::parse(path)?;
        if path.is_root() {
            return Err(FsError::PermissionDenied);
        }
        let name = path.file_name().ok_or(FsError::InvalidPath)?.to_string();
        let identity = self.identity;
        let parent = self.find_parent_mut(&path).ok_or(FsError::NotFound)?;
        match parent {
            Node::Directory {
                permissions,
                entries,
            } => {
                if !permissions.allows(&identity, Access::Write) {
                    return Err(FsError::PermissionDenied);
                }
                let node = entries
                    .iter()
                    .find(|(entry_name, _)| *entry_name == name)
                    .map(|(_, node)| node)
                    .ok_or(FsError::NotFound)?;
                if let Node::Directory { entries, .. } = node {
                    if !entries.is_empty() {
                        return Err(FsError::NotEmpty);
                    }
                }
                parent.remove(&name);
                Ok(())
            }
            Node::File { .. } => Err(FsError::NotADirectory),
        }
    }

    fn read_dir(&mut self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        if !self.mounted {
            return Err(FsError::NotMounted);
        }
        let path = Path::parse(path)?;
        let node = self.find(&path).ok_or(FsError::NotFound)?;
        let Node::Directory {
            entries,
            permissions,
        } = node
        else {
            return Err(FsError::NotADirectory);
        };
        if !permissions.allows(&self.identity, Access::Read) {
            return Err(FsError::PermissionDenied);
        }
        Ok(entries
            .iter()
            .map(|(name, node)| DirEntry {
                name: name.clone(),
                file_type: node.file_type(),
            })
            .collect())
    }

    fn stat(&mut self, path: &str) -> Result<FileMetadata, FsError> {
        if !self.mounted {
            return Err(FsError::NotMounted);
        }
        let path = Path::parse(path)?;
        let node = self.find(&path).ok_or(FsError::NotFound)?;
        let permissions = node.permissions();
        Ok(FileMetadata {
            file_type: node.file_type(),
            size: node.size(),
            owner: permissions.owner,
            permissions,
        })
    }

    fn set_permissions(&mut self, path: &str, permissions: Permissions) -> Result<(), FsError> {
        if !self.mounted {
            return Err(FsError::NotMounted);
        }
        let path = Path::parse(path)?;
        let node = self.find_mut(&path).ok_or(FsError::NotFound)?;
        match node {
            Node::Directory {
                permissions: current,
                ..
            } => *current = permissions,
            Node::File {
                permissions: current,
                ..
            } => *current = permissions,
        }
        Ok(())
    }

    fn exists(&mut self, path: &str) -> bool {
        if !self.mounted {
            return false;
        }
        match Path::parse(path) {
            Ok(path) => self.find(&path).is_some(),
            Err(_) => false,
        }
    }

    fn open_handle_count(&self) -> usize {
        self.handles.iter().filter(|slot| slot.is_some()).count()
    }
}

/// Write the permissions of an object into an image.
fn write_permissions(image: &mut Vec<u8>, permissions: Permissions) {
    match permissions.owner {
        Owner::System => image.push(OWNER_SYSTEM),
        Owner::User(uid) => {
            image.push(OWNER_USER);
            image.extend_from_slice(&uid.to_le_bytes());
        }
        Owner::UserGroup { uid, .. } => {
            image.push(OWNER_USER);
            image.extend_from_slice(&uid.to_le_bytes());
        }
    }
    let mut bits = 0u8;
    if permissions.read {
        bits |= 1;
    }
    if permissions.write {
        bits |= 2;
    }
    if permissions.execute {
        bits |= 4;
    }
    image.push(bits);
}

/// Read the permissions of an object out of an image.
fn read_permissions(bytes: &[u8], offset: &mut usize) -> Result<Permissions, FsError> {
    let owner = match *bytes.get(*offset).ok_or(FsError::IoError)? {
        OWNER_SYSTEM => {
            *offset += 1;
            Owner::System
        }
        OWNER_USER => {
            *offset += 1;
            let uid = read_u64(bytes, offset)?;
            Owner::User(uid)
        }
        _ => return Err(FsError::IoError),
    };
    let bits = *bytes.get(*offset).ok_or(FsError::IoError)?;
    *offset += 1;
    Ok(Permissions {
        owner,
        read: bits & 1 != 0,
        write: bits & 2 != 0,
        execute: bits & 4 != 0,
    })
}

/// Read a little-endian number of four bytes out of an image.
fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, FsError> {
    if *offset + 4 > bytes.len() {
        return Err(FsError::IoError);
    }
    let value = u32::from_le_bytes([
        bytes[*offset],
        bytes[*offset + 1],
        bytes[*offset + 2],
        bytes[*offset + 3],
    ]);
    *offset += 4;
    Ok(value)
}

/// Read a little-endian number of eight bytes out of an image.
fn read_u64(bytes: &[u8], offset: &mut usize) -> Result<u64, FsError> {
    if *offset + 8 > bytes.len() {
        return Err(FsError::IoError);
    }
    let mut value = [0u8; 8];
    value.copy_from_slice(&bytes[*offset..*offset + 8]);
    *offset += 8;
    Ok(u64::from_le_bytes(value))
}

/// Put an object into the tree, creating the directories it needs.
fn insert_into_tree(root: &mut Node, path: &str, node: Node) -> Result<(), FsError> {
    let parsed = Path::parse(path).map_err(FsError::from)?;
    if parsed.is_root() {
        return Err(FsError::InvalidPath);
    }
    let name = parsed.file_name().ok_or(FsError::InvalidPath)?.to_string();
    let parent_path = parsed.parent().ok_or(FsError::InvalidPath)?;

    let mut current = root;
    for component in parent_path.components() {
        if current.entry(component).is_none() {
            current.insert(
                component,
                Node::Directory {
                    entries: Vec::new(),
                    permissions: Permissions::directory(),
                },
            )?;
        }
        current = current.entry_mut(component).ok_or(FsError::NotFound)?;
    }
    current.insert(&name, node)
}

/// Translate a block error into a filesystem error.
fn map_block_error(error: BlockError) -> FsError {
    match error {
        BlockError::OutOfRange => FsError::IoError,
        BlockError::BadBufferSize => FsError::IoError,
        BlockError::IoError => FsError::IoError,
        BlockError::NotReady => FsError::IoError,
        BlockError::DeviceBusy => FsError::IoError,
    }
}
