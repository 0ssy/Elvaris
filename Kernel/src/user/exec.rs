//! Running a program that the filesystem holds.
//!
//! The kernel owns exactly one user window, so only one program can be
//! resident at a time. A program that starts another one therefore works in
//! four steps:
//!
//! 1. The window is copied into the kernel heap.
//! 2. The child program is loaded into the window and started.
//! 3. The child program runs until it ends.
//! 4. The window is written back, so that the caller finds itself exactly
//!    where it was.
//!
//! # The stack of the events
//!
//! The processor switches to the stack of the task state segment whenever an
//! event arrives from user mode. That stack belongs to the program that is
//! running: a program that starts another one cannot use the same stack,
//! because the events of the child program would overwrite the frames of the
//! caller. The kernel therefore gives every nested program a stack of its
//! own and puts the stack of the caller back when the child program ends.
//!
//! # Limitations
//!
//! - The child program runs to completion: there is no scheduling, and the
//!   caller waits.
//! - The window is copied in and out for every program that is started.
//! - The nesting depth is limited by the memory the kernel heap has.

use alloc::vec::Vec;

use crate::cpu::tss;
use crate::fs::FileSystem;
use crate::user::argv::Arguments;
use crate::user::loader::{self, LoadError};
use crate::user::session::{self, Outcome};

/// The size of the stack the events of a nested program are handled on.
///
/// The stack belongs to the kernel: the user program never touches it.
pub const NESTED_INTERRUPT_STACK_SIZE: usize = 16 * 1024;

/// The result of running a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecResult {
    /// How the program ended.
    pub outcome: Outcome,
    /// The number of system calls the program made.
    pub syscalls: u64,
}

/// The exit code the kernel reports for a program that did not end normally.
///
/// A program that caused an exception or that the kernel refused is reported
/// with this code, so that the caller can tell it apart from a program that
/// ended with the code it asked for.
pub const ABNORMAL_EXIT_CODE: u64 = 255;

/// The exit code of a program, or [`ABNORMAL_EXIT_CODE`] when the program did
/// not end normally.
pub fn exit_code(outcome: Outcome) -> u64 {
    match outcome {
        Outcome::Exited(code) => code & 0xFF,
        _ => ABNORMAL_EXIT_CODE,
    }
}

/// A short description of the reason a program could not be loaded.
pub fn load_error_description(error: LoadError) -> &'static str {
    match error {
        LoadError::NotFound => "there is no such program",
        LoadError::NotAFile => "the path does not name a file",
        LoadError::PermissionDenied => "the program may not be run",
        LoadError::IoError => "the medium failed",
        LoadError::InvalidImage(_) => "the image is not a program image",
        LoadError::ProgramTooLarge => "the program does not fit into the code page",
        LoadError::ArgumentsTooLarge => "the arguments do not fit into the data page",
        LoadError::OutOfMemory => "the kernel heap is full",
        LoadError::Mapping(_) => "the virtual memory subsystem refused the mapping",
        LoadError::InvalidPath => "the path is not valid",
    }
}

/// A program that is ready to be run.
///
/// The preparation and the run are two separate steps, because the kernel
/// must not hold the filesystem while a program runs: the program itself
/// uses the filesystem, and the lock of the filesystem is not recursive.
pub struct Prepared {
    /// The image of the program.
    image: Vec<u8>,
    /// The arguments of the program.
    arguments: Arguments,
}

impl Prepared {
    /// The image of the program.
    pub fn image(&self) -> &[u8] {
        &self.image
    }

    /// The arguments of the program.
    pub fn arguments(&self) -> &Arguments {
        &self.arguments
    }
}

/// Read a program out of the filesystem and prepare it.
///
/// The function resolves the path of the program and reads its image. The
/// caller must not hold the filesystem while it runs the program.
pub fn prepare(
    fs: &mut dyn FileSystem,
    path: &str,
    arguments: &str,
) -> Result<Prepared, LoadError> {
    let (resolved, image) = loader::read_program(fs, path)?;
    Ok(Prepared {
        image,
        arguments: Arguments::new(&resolved, arguments),
    })
}

/// Run a prepared program and return when it ends.
///
/// The window of the caller is saved before the program is loaded and
/// restored when it ends, so the caller finds itself exactly where it was.
pub fn run(program: &Prepared) -> Result<ExecResult, LoadError> {
    // The window of the caller is saved before the child program is loaded.
    let saved = loader::save_window()?;
    let loaded = loader::install(&program.image, &program.arguments)?;

    // The events of the program need a stack of their own.
    let stack =
        crate::heap::alloc(NESTED_INTERRUPT_STACK_SIZE, 16).map_err(|_| LoadError::OutOfMemory)?;
    let stack_top = stack.as_ptr() as u64 + NESTED_INTERRUPT_STACK_SIZE as u64;
    let previous_stack = tss::kernel_stack_top();
    tss::set_privilege_stack(stack_top);

    let outcome = session::run(
        loaded.entry,
        loaded.stack_pointer,
        loaded.argc,
        loaded.argv,
        loaded.block,
        loaded.block_length,
    );
    let syscalls = session::last_syscalls();

    tss::set_privilege_stack(previous_stack);
    let _ = crate::heap::dealloc(stack, NESTED_INTERRUPT_STACK_SIZE);

    // The caller finds itself exactly where it was.
    loader::restore_window(&saved)?;

    Ok(ExecResult { outcome, syscalls })
}

/// Read a program out of the filesystem and run it.
///
/// The function is the convenience the kernel uses for the first program of
/// the system. The caller must not hold the filesystem while it runs.
pub fn run_program(
    fs: &mut dyn FileSystem,
    path: &str,
    arguments: &str,
) -> Result<ExecResult, LoadError> {
    let program = prepare(fs, path, arguments)?;
    run(&program)
}

/// Run a program out of the filesystem and return its exit code.
///
/// The function is the convenience the system call handler uses: it hides
/// the outcome of the program behind the number the caller expects.
pub fn run_program_for_exit_code(
    fs: &mut dyn FileSystem,
    path: &str,
    arguments: &str,
) -> Result<u64, LoadError> {
    let result = run_program(fs, path, arguments)?;
    Ok(exit_code(result.outcome))
}
