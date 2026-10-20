//! Unit tests for the filesystem interface, the path rules and the
//! in-memory filesystem.
//!
//! The tests run on the host. They never touch a device, a page table or the
//! console: the memory-backed block device stands in for the medium.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::memfs::MemFs;
use super::path::{MAX_NAME_LENGTH, MAX_PATH_LENGTH, Path, PathError};
use super::{
    Access, FileHandle, FileSystem, FileType, FsError, Identity, OpenFlags, Owner, Permissions,
};

// ---------------------------------------------------------------------------
// The path rules
// ---------------------------------------------------------------------------

#[test]
fn the_root_is_a_single_separator() {
    let path = Path::parse("/").expect("the root is a path");
    assert!(path.is_root());
    assert_eq!(path.len(), 0);
    assert_eq!(path.to_string(), "/");
}

#[test]
fn an_empty_path_is_refused() {
    assert_eq!(Path::parse(""), Err(PathError::Empty));
}

#[test]
fn a_path_is_split_into_components() {
    let path = Path::parse("/bin/hello").expect("the path is valid");
    assert_eq!(path.components().len(), 2);
    assert_eq!(path.components()[0], "bin");
    assert_eq!(path.components()[1], "hello");
    assert_eq!(path.file_name(), Some("hello"));
    assert_eq!(path.to_string(), "/bin/hello");
}

#[test]
fn repeated_separators_are_collapsed() {
    assert_eq!(
        Path::parse("/bin//hello")
            .expect("the path is valid")
            .to_string(),
        "/bin/hello"
    );
    assert_eq!(
        Path::parse("//bin///hello//")
            .expect("the path is valid")
            .to_string(),
        "/bin/hello"
    );
}

#[test]
fn a_trailing_separator_is_ignored() {
    assert_eq!(
        Path::parse("/bin/hello/")
            .expect("the path is valid")
            .to_string(),
        "/bin/hello"
    );
}

#[test]
fn the_current_directory_does_not_change_the_path() {
    assert_eq!(
        Path::parse("/bin/./hello")
            .expect("the path is valid")
            .to_string(),
        "/bin/hello"
    );
    assert_eq!(
        Path::parse("/.").expect("the path is valid").to_string(),
        "/"
    );
}

#[test]
fn the_parent_directory_is_resolved() {
    assert_eq!(
        Path::parse("/bin/../hello")
            .expect("the path is valid")
            .to_string(),
        "/hello"
    );
    assert_eq!(
        Path::parse("/bin/sh/../hello")
            .expect("the path is valid")
            .to_string(),
        "/bin/hello"
    );
}

#[test]
fn the_parent_of_the_root_is_the_root() {
    assert_eq!(
        Path::parse("/..").expect("the path is valid").to_string(),
        "/"
    );
    assert_eq!(
        Path::parse("/../..")
            .expect("the path is valid")
            .to_string(),
        "/"
    );
}

#[test]
fn a_relative_path_is_resolved_against_the_root() {
    assert_eq!(
        Path::parse("bin/hello")
            .expect("the path is valid")
            .to_string(),
        "/bin/hello"
    );
    assert_eq!(
        Path::parse("hello").expect("the path is valid").to_string(),
        "/hello"
    );
}

#[test]
fn a_path_that_holds_a_nul_byte_is_refused() {
    assert_eq!(Path::parse("/bin\0hello"), Err(PathError::InvalidCharacter));
}

#[test]
fn a_path_that_is_too_long_is_refused() {
    let long = format!("/{}", "a".repeat(MAX_PATH_LENGTH));
    assert_eq!(Path::parse(&long), Err(PathError::TooLong));
}

#[test]
fn a_component_that_is_too_long_is_refused() {
    let long = format!("/{}", "a".repeat(MAX_NAME_LENGTH + 1));
    assert_eq!(Path::parse(&long), Err(PathError::InvalidComponent));
}

#[test]
fn the_parent_of_a_path_is_the_path_without_its_name() {
    let path = Path::parse("/bin/hello").expect("the path is valid");
    let parent = path.parent().expect("the path has a parent");
    assert_eq!(parent.to_string(), "/bin");
    assert!(
        Path::parse("/")
            .expect("the root is a path")
            .parent()
            .is_none()
    );
}

#[test]
fn a_name_can_be_appended_to_a_path() {
    let path = Path::parse("/bin").expect("the path is valid");
    let child = path.join("hello").expect("the name is valid");
    assert_eq!(child.to_string(), "/bin/hello");
    assert_eq!(path.join(""), Err(PathError::InvalidComponent));
    assert_eq!(path.join(".."), Err(PathError::InvalidComponent));
    assert_eq!(path.join("a/b"), Err(PathError::InvalidComponent));
}

// ---------------------------------------------------------------------------
// The in-memory filesystem
// ---------------------------------------------------------------------------

/// A mounted filesystem with an empty root.
fn mounted() -> MemFs {
    let mut fs = MemFs::new();
    fs.mount().expect("the filesystem mounts");
    fs
}

#[test]
fn a_filesystem_is_not_usable_before_it_is_mounted() {
    let mut fs = MemFs::new();
    assert!(!fs.is_mounted());
    assert_eq!(
        fs.create_file("/a", Permissions::public_read()),
        Err(FsError::NotMounted)
    );
    assert_eq!(fs.open("/a", OpenFlags::read()), Err(FsError::NotMounted));
    assert_eq!(fs.read_dir("/"), Err(FsError::NotMounted));
    assert!(!fs.exists("/a"));
}

#[test]
fn a_filesystem_cannot_be_mounted_twice() {
    let mut fs = mounted();
    assert_eq!(fs.mount(), Err(FsError::AlreadyExists));
}

#[test]
fn a_file_can_be_created_and_found() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::public_read())
        .expect("the file is created");

    assert!(fs.exists("/hello"));
    let metadata = fs.stat("/hello").expect("the file has metadata");
    assert_eq!(metadata.file_type, FileType::File);
    assert_eq!(metadata.size, 0);
    assert_eq!(metadata.permissions.owner, Owner::System);
}

#[test]
fn a_file_that_does_not_exist_is_reported() {
    let mut fs = mounted();
    assert_eq!(fs.stat("/nope"), Err(FsError::NotFound));
    assert_eq!(fs.open("/nope", OpenFlags::read()), Err(FsError::NotFound));
    assert_eq!(fs.delete("/nope"), Err(FsError::NotFound));
}

#[test]
fn a_file_cannot_be_created_twice() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::public_read())
        .expect("the file is created");
    assert_eq!(
        fs.create_file("/hello", Permissions::public_read()),
        Err(FsError::AlreadyExists)
    );
}

#[test]
fn a_file_needs_a_parent_that_exists() {
    let mut fs = mounted();
    assert_eq!(
        fs.create_file("/bin/hello", Permissions::public_read()),
        Err(FsError::NotFound)
    );
    fs.create_dir("/bin", Permissions::directory())
        .expect("the directory is created");
    fs.create_file("/bin/hello", Permissions::public_read())
        .expect("the file is created");
}

#[test]
fn a_directory_can_be_created_and_listed() {
    let mut fs = mounted();
    fs.create_dir("/bin", Permissions::directory())
        .expect("the directory is created");
    fs.create_file("/bin/hello", Permissions::public_read())
        .expect("the file is created");

    let entries = fs.read_dir("/bin").expect("the directory is listed");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "hello");
    assert_eq!(entries[0].file_type, FileType::File);
}

#[test]
fn a_file_cannot_be_listed_as_a_directory() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::public_read())
        .expect("the file is created");
    assert_eq!(fs.read_dir("/hello"), Err(FsError::NotADirectory));
}

#[test]
fn a_directory_cannot_be_opened_as_a_file() {
    let mut fs = mounted();
    fs.create_dir("/bin", Permissions::directory())
        .expect("the directory is created");
    assert_eq!(
        fs.open("/bin", OpenFlags::read()),
        Err(FsError::IsADirectory)
    );
}

#[test]
fn a_file_can_be_written_and_read_back() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::system_write())
        .expect("the file is created");

    let handle = fs
        .open("/hello", OpenFlags::read_write())
        .expect("the file opens");
    let written = fs
        .write(handle, b"Hello from Elvaris!")
        .expect("the write succeeds");
    assert_eq!(written, 19);

    let mut buffer = [0u8; 32];
    fs.seek(handle, 0).expect("the seek succeeds");
    let read = fs.read(handle, &mut buffer).expect("the read succeeds");
    assert_eq!(&buffer[..read], b"Hello from Elvaris!");
    fs.close(handle).expect("the handle is released");
}

#[test]
fn reading_at_the_end_of_a_file_returns_nothing() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::system_write())
        .expect("the file is created");
    let handle = fs
        .open("/hello", OpenFlags::read_write())
        .expect("the file opens");
    fs.write(handle, b"abc").expect("the write succeeds");
    fs.seek(handle, 0).expect("the seek succeeds");

    let mut buffer = [0u8; 8];
    let read = fs.read(handle, &mut buffer).expect("the read succeeds");
    assert_eq!(read, 3);
    let read = fs.read(handle, &mut buffer).expect("the read succeeds");
    assert_eq!(read, 0);
}

#[test]
fn the_position_of_a_file_moves_with_every_transfer() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::system_write())
        .expect("the file is created");
    let handle = fs
        .open("/hello", OpenFlags::read_write())
        .expect("the file opens");
    fs.write(handle, b"abc").expect("the write succeeds");

    let mut buffer = [0u8; 8];
    fs.read(handle, &mut buffer).expect("the read succeeds");
    let read = fs.read(handle, &mut buffer).expect("the read succeeds");
    assert_eq!(read, 0);
}

#[test]
fn a_file_cannot_be_read_when_it_was_opened_for_writing() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::system_write())
        .expect("the file is created");
    let handle = fs
        .open("/hello", OpenFlags::write())
        .expect("the file opens");

    let mut buffer = [0u8; 8];
    assert_eq!(fs.read(handle, &mut buffer), Err(FsError::PermissionDenied));
}

#[test]
fn a_file_cannot_be_written_when_it_was_opened_for_reading() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::system_write())
        .expect("the file is created");
    let handle = fs
        .open("/hello", OpenFlags::read())
        .expect("the file opens");

    assert_eq!(fs.write(handle, b"abc"), Err(FsError::PermissionDenied));
}

#[test]
fn a_handle_can_be_released_only_once() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::public_read())
        .expect("the file is created");
    let handle = fs
        .open("/hello", OpenFlags::read())
        .expect("the file opens");
    assert_eq!(fs.open_handle_count(), 1);

    fs.close(handle).expect("the handle is released");
    assert_eq!(fs.open_handle_count(), 0);
    assert_eq!(fs.close(handle), Err(FsError::InvalidArgument));
}

#[test]
fn an_unknown_handle_is_refused() {
    let mut fs = mounted();
    let handle = FileHandle(9999);
    let mut buffer = [0u8; 4];
    assert_eq!(fs.read(handle, &mut buffer), Err(FsError::InvalidArgument));
    assert_eq!(fs.close(handle), Err(FsError::InvalidArgument));
}

#[test]
fn a_duplicated_handle_has_its_own_position() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::system_write())
        .expect("the file is created");
    let handle = fs
        .open("/hello", OpenFlags::read_write())
        .expect("the file opens");
    fs.write(handle, b"abcdef").expect("the write succeeds");

    let copy = fs.duplicate(handle).expect("the handle is duplicated");
    let mut buffer = [0u8; 8];
    let read = fs.read(copy, &mut buffer).expect("the read succeeds");
    assert_eq!(&buffer[..read], b"abcdef");
    assert_eq!(fs.open_handle_count(), 2);
}

#[test]
fn a_file_can_be_deleted() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::public_read())
        .expect("the file is created");
    fs.delete("/hello").expect("the file is deleted");
    assert!(!fs.exists("/hello"));
}

#[test]
fn a_directory_that_is_not_empty_cannot_be_deleted() {
    let mut fs = mounted();
    fs.create_dir("/bin", Permissions::directory())
        .expect("the directory is created");
    fs.create_file("/bin/hello", Permissions::public_read())
        .expect("the file is created");

    assert_eq!(fs.delete("/bin"), Err(FsError::NotEmpty));
    fs.delete("/bin/hello").expect("the file is deleted");
    fs.delete("/bin").expect("the directory is deleted");
    assert!(!fs.exists("/bin"));
}

#[test]
fn opening_a_file_that_does_not_exist_can_create_it() {
    let mut fs = mounted();
    let handle = fs
        .open("/new", OpenFlags::create())
        .expect("the file is created");
    fs.close(handle).expect("the handle is released");
    assert!(fs.exists("/new"));
}

#[test]
fn opening_a_file_with_the_truncate_flag_drops_its_content() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::system_write())
        .expect("the file is created");
    let handle = fs
        .open("/hello", OpenFlags::read_write())
        .expect("the file opens");
    fs.write(handle, b"abcdef").expect("the write succeeds");
    fs.close(handle).expect("the handle is released");

    let handle = fs
        .open("/hello", OpenFlags::truncate())
        .expect("the file opens");
    fs.close(handle).expect("the handle is released");

    let metadata = fs.stat("/hello").expect("the file has metadata");
    assert_eq!(metadata.size, 0);
}

#[test]
fn the_permissions_of_a_file_are_refused_when_they_are_not_allowed() {
    let mut fs = mounted();
    fs.create_file("/secret", Permissions::public_read())
        .expect("the file is created");

    // The file may be read but not written.
    assert_eq!(
        fs.open("/secret", OpenFlags::write()),
        Err(FsError::PermissionDenied)
    );
    assert!(fs.open("/secret", OpenFlags::read()).is_ok());
}

#[test]
fn a_file_of_another_user_cannot_be_used() {
    let mut fs = mounted();
    fs.create_file(
        "/private",
        Permissions {
            owner: Owner::User(7),
            read: true,
            write: true,
            execute: false,
        },
    )
    .expect("the file is created");

    // The filesystem acts as the system, which is not user seven.
    assert_eq!(
        fs.open("/private", OpenFlags::read()),
        Err(FsError::PermissionDenied)
    );

    // The owner may use the file.
    fs.set_identity(Identity { uid: 7, gid: 7 });
    assert!(fs.open("/private", OpenFlags::read()).is_ok());
}

#[test]
fn the_permissions_of_an_object_can_be_changed() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::public_read())
        .expect("the file is created");
    assert_eq!(
        fs.open("/hello", OpenFlags::read_write()),
        Err(FsError::PermissionDenied)
    );

    fs.set_permissions("/hello", Permissions::system_write())
        .expect("the permissions are changed");
    assert!(fs.open("/hello", OpenFlags::read_write()).is_ok());
}

#[test]
fn the_image_of_a_filesystem_can_be_written_and_read_back() {
    let mut fs = mounted();
    fs.create_dir("/bin", Permissions::directory())
        .expect("the directory is created");
    fs.create_file("/bin/hello", Permissions::program())
        .expect("the file is created");
    let handle = fs
        .open("/bin/hello", OpenFlags::read_write_create())
        .expect("the file opens");
    fs.write(handle, b"program").expect("the write succeeds");
    fs.close(handle).expect("the handle is released");

    let image = fs.image();
    assert!(image.len() > 8);

    let mut other = MemFs::new();
    other.install_image(&image).expect("the image is installed");
    other.mount().expect("the filesystem mounts");

    assert!(other.exists("/bin"));
    assert!(other.exists("/bin/hello"));

    let handle = other
        .open("/bin/hello", OpenFlags::read())
        .expect("the file opens");
    let mut buffer = [0u8; 16];
    let read = other.read(handle, &mut buffer).expect("the read succeeds");
    assert_eq!(&buffer[..read], b"program");
}

#[test]
fn an_image_that_is_not_an_image_is_refused() {
    let mut fs = MemFs::new();
    fs.install_image(b"not an image at all, but long enough")
        .expect("the image is installed");
    assert_eq!(fs.mount(), Err(FsError::IoError));
}

#[test]
fn a_filesystem_can_be_unmounted_and_mounted_again() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::public_read())
        .expect("the file is created");
    fs.unmount().expect("the filesystem unmounts");
    assert!(!fs.is_mounted());
    assert_eq!(fs.unmount(), Err(FsError::NotMounted));

    fs.mount().expect("the filesystem mounts again");
    assert!(fs.exists("/hello"));
}

#[test]
fn the_names_of_the_entries_are_kept() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::public_read())
        .expect("the file is created");
    let entries = fs.read_dir("/").expect("the root is listed");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "hello");
    assert_eq!(entries[0].file_type, FileType::File);
}

#[test]
fn a_long_name_is_accepted_up_to_the_limit() {
    let mut fs = mounted();
    let name = "a".repeat(MAX_NAME_LENGTH);
    let path = format!("/{name}");
    fs.create_file(&path, Permissions::public_read())
        .expect("the file is created");
    assert!(fs.exists(&path));
}

#[test]
fn the_size_of_a_file_follows_its_content() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::system_write())
        .expect("the file is created");
    let handle = fs
        .open("/hello", OpenFlags::read_write())
        .expect("the file opens");
    fs.write(handle, b"12345").expect("the write succeeds");
    let metadata = fs.stat("/hello").expect("the file has metadata");
    assert_eq!(metadata.size, 5);
}

#[test]
fn an_empty_directory_has_no_entries() {
    let mut fs = mounted();
    fs.create_dir("/empty", Permissions::directory())
        .expect("the directory is created");
    let entries = fs.read_dir("/empty").expect("the directory is listed");
    assert!(entries.is_empty());
}

#[test]
fn a_name_is_a_string_that_can_be_compared() {
    let mut fs = mounted();
    fs.create_file("/hello", Permissions::public_read())
        .expect("the file is created");
    let entries = fs.read_dir("/").expect("the root is listed");
    let names: Vec<String> = entries.into_iter().map(|entry| entry.name).collect();
    assert_eq!(names, vec![String::from("hello")]);
    assert!(
        Path::parse("/hello")
            .expect("the path is valid")
            .is_below(&Path::parse("/").expect("the root is a path"))
    );
}
