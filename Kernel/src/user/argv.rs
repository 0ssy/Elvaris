//! The arguments of a user program.
//!
//! A program is started with a list of arguments: the path of the program
//! itself and the words that followed it on the command line. The kernel
//! builds the list, the loader puts it into the address space of the
//! program, and the program reads it.
//!
//! # The model
//!
//! The arguments are the words of a command line, separated by spaces or
//! tabs. The first argument is always the path of the program; the rest are
//! the words that the caller passed.
//!
//! ```text
//!  /bin/hello one two    ->  ["/bin/hello", "one", "two"]
//! ```
//!
//! The module is pure: it never touches a device, a page table or the
//! console, so every rule can be checked by a host test.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// The maximum number of arguments a program may get.
pub const MAX_ARGUMENTS: usize = 32;

/// The maximum length of one argument, without the terminating NUL byte.
pub const MAX_ARGUMENT_LENGTH: usize = 256;

/// The characters that separate the words of a command line.
const SEPARATORS: [char; 2] = [' ', '\t'];

/// The arguments of a program.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Arguments {
    entries: Vec<String>,
}

impl Arguments {
    /// Build the arguments of a program.
    ///
    /// The path of the program is the first argument. The rest are the words
    /// of `arguments`. The list is cut at [`MAX_ARGUMENTS`] entries, and
    /// every entry is cut at [`MAX_ARGUMENT_LENGTH`] bytes.
    pub fn new(program: &str, arguments: &str) -> Self {
        let mut entries = Vec::new();
        entries.push(cut(program));
        for word in tokenize(arguments) {
            if entries.len() >= MAX_ARGUMENTS {
                break;
            }
            entries.push(cut(&word));
        }
        Arguments { entries }
    }

    /// Build the arguments out of a list of words.
    pub fn from_words(words: &[&str]) -> Self {
        let mut entries = Vec::new();
        for word in words {
            if entries.len() >= MAX_ARGUMENTS {
                break;
            }
            entries.push(cut(word));
        }
        Arguments { entries }
    }

    /// The number of arguments.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the program has no argument at all.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The argument with that index.
    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|entry| entry.as_str())
    }

    /// The arguments, in order.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// The size of the block of strings, in bytes.
    ///
    /// Every string of the block ends with a NUL byte, and the block ends
    /// with an extra NUL byte.
    pub fn block_size(&self) -> usize {
        self.entries
            .iter()
            .map(|entry| entry.len() + 1)
            .sum::<usize>()
            + 1
    }

    /// The size of the table of the pointers to the strings, in bytes.
    ///
    /// The table holds one pointer per argument and one null pointer that
    /// ends it.
    pub fn table_size(&self) -> usize {
        (self.entries.len() + 1) * 8
    }
}

/// Split a command line into its words.
///
/// The words are separated by spaces or tabs. Empty words are dropped, so
/// that a command line with many spaces between the words still produces
/// the same list.
pub fn tokenize(text: &str) -> Vec<String> {
    text.split(SEPARATORS)
        .filter(|word| !word.is_empty())
        .map(|word| word.to_string())
        .collect()
}

/// Cut a word at the maximum length.
fn cut(word: &str) -> String {
    if word.len() <= MAX_ARGUMENT_LENGTH {
        return word.to_string();
    }
    let mut end = MAX_ARGUMENT_LENGTH;
    while end > 0 && !word.is_char_boundary(end) {
        end -= 1;
    }
    word[..end].to_string()
}
