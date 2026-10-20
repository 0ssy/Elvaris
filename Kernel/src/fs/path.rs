//! The rules for the paths of the filesystem.
//!
//! Every path the kernel accepts goes through this module: the paths of the
//! system calls, the paths of the program loader and the paths of the
//! filesystem itself. The module is pure: it never touches a device, a page
//! table or the console, so every rule can be checked by a host test.
//!
//! # The rules
//!
//! - `/` is the separator and the root of the filesystem.
//! - A path is a sequence of components separated by `/`.
//! - An empty path is not a path.
//! - Repeated separators are collapsed: `/bin//sh` is `/bin/sh`.
//! - `.` is the current directory and never changes the path.
//! - `..` is the parent directory. At the root it stays at the root:
//!   `/..` is `/`.
//! - A relative path is resolved against the root, because the kernel has no
//!   per-process current directory yet.
//! - A component may not be empty, may not hold a NUL byte and may not be
//!   longer than [`MAX_NAME_LENGTH`].
//! - A path may not be longer than [`MAX_PATH_LENGTH`].
//!
//! # The canonical form
//!
//! [`Path::parse`] returns the canonical form of a path: it is always
//! absolute, it never holds an empty component, and it never holds `.` or
//! `..`. Two paths that name the same object have the same canonical form.

use alloc::string::String;
use alloc::vec::Vec;

/// The longest path the kernel accepts.
pub const MAX_PATH_LENGTH: usize = 1024;

/// The longest name of one component the kernel accepts.
pub const MAX_NAME_LENGTH: usize = 255;

/// The separator between the components of a path.
pub const SEPARATOR: char = '/';

/// The reasons a path is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathError {
    /// The path is longer than [`MAX_PATH_LENGTH`].
    TooLong,
    /// The path is empty.
    Empty,
    /// The path holds a character that a path may not hold.
    InvalidCharacter,
    /// A component of the path is not usable as a name.
    InvalidComponent,
}

/// A path in its canonical form.
///
/// The canonical form is absolute, has no empty component and has no `.` or
/// `..` component. The root is the path without any component.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Path {
    components: Vec<String>,
}

impl Path {
    /// The root of the filesystem.
    pub fn root() -> Self {
        Path {
            components: Vec::new(),
        }
    }

    /// Parse a path and return its canonical form.
    ///
    /// The function never fails because of the content of a component: a
    /// component that is `.` or `..` is resolved, not refused.
    pub fn parse(text: &str) -> Result<Self, PathError> {
        if text.is_empty() {
            return Err(PathError::Empty);
        }
        if text.len() > MAX_PATH_LENGTH {
            return Err(PathError::TooLong);
        }
        if text.contains('\0') {
            return Err(PathError::InvalidCharacter);
        }

        let mut components: Vec<String> = Vec::new();
        for component in text.split(SEPARATOR) {
            // An empty component comes from a repeated separator or from a
            // trailing separator. Both are collapsed.
            if component.is_empty() {
                continue;
            }
            if component == "." {
                continue;
            }
            if component == ".." {
                // The parent of the root is the root.
                components.pop();
                continue;
            }
            if component.len() > MAX_NAME_LENGTH {
                return Err(PathError::InvalidComponent);
            }
            if component.contains('\0') {
                return Err(PathError::InvalidCharacter);
            }
            components.push(String::from(component));
        }

        Ok(Path { components })
    }

    /// The components of the path, from the root down.
    pub fn components(&self) -> &[String] {
        &self.components
    }

    /// Whether the path is the root.
    pub fn is_root(&self) -> bool {
        self.components.is_empty()
    }

    /// The number of components of the path.
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Whether the path has no component.
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    /// The last component of the path.
    ///
    /// The root has no name.
    pub fn file_name(&self) -> Option<&str> {
        self.components.last().map(|name| name.as_str())
    }

    /// The path of the parent directory.
    ///
    /// The root has no parent.
    pub fn parent(&self) -> Option<Path> {
        if self.components.is_empty() {
            return None;
        }
        let mut components = self.components.clone();
        components.pop();
        Some(Path { components })
    }

    /// Append a name to the path.
    ///
    /// The name may not be empty, may not hold a separator and may not be
    /// `.` or `..`.
    pub fn join(&self, name: &str) -> Result<Path, PathError> {
        if name.is_empty() {
            return Err(PathError::InvalidComponent);
        }
        if name.len() > MAX_NAME_LENGTH {
            return Err(PathError::InvalidComponent);
        }
        if name.contains('\0') {
            return Err(PathError::InvalidCharacter);
        }
        if name.contains(SEPARATOR) {
            return Err(PathError::InvalidComponent);
        }
        if name == "." || name == ".." {
            return Err(PathError::InvalidComponent);
        }
        let mut components = self.components.clone();
        components.push(String::from(name));
        Ok(Path { components })
    }

    /// The canonical form of the path as a string.
    ///
    /// The root is `/`.
    fn as_text(&self) -> String {
        if self.components.is_empty() {
            return String::from("/");
        }
        let mut text = String::new();
        for component in &self.components {
            text.push(SEPARATOR);
            text.push_str(component);
        }
        text
    }

    /// Whether the path is below the given directory.
    ///
    /// A directory is below itself.
    pub fn is_below(&self, directory: &Path) -> bool {
        if directory.components.len() > self.components.len() {
            return false;
        }
        self.components[..directory.components.len()] == directory.components[..]
    }
}

impl core::fmt::Display for Path {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.as_text())
    }
}

/// The canonical form of a path, or [`FsError::InvalidPath`](crate::fs::FsError).
///
/// The function is a convenience for the callers that already work with
/// [`crate::fs::FsError`].
pub fn canonicalize(text: &str) -> Result<String, PathError> {
    Ok(Path::parse(text)?.as_text())
}
