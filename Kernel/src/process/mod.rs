//! Process table and run loop.
//!
//! # The model of this milestone
//!
//! The kernel owns one address space and one user window, so only one program
//! runs at a time. A program that starts another one blocks in the system
//! call until the program it started ends. The table of the processes is
//! therefore a record of what ran, not a scheduler.
//!
//! The table gives every program an identifier, remembers the program it
//! came from, and remembers how it ended. The shell reports the table, so
//! the state of the machine is visible from the outside.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::Mutex;

use crate::process::process::{Process, ProcessId};
use crate::serial;

pub use crate::process::process::{
    AddressSpace, ExitStatus, FaultInfo, ProcessState, ProgramDescriptor, RejectInfo,
    ResourceTable, SecurityIdentity, UserThread, UserThreadState,
};

use crate::security::{Credentials, ROOT_UID, ROOT_GID, USER_UID, USER_GID};

pub const MAX_PROCESSES: usize = 16;

/// The table of the processes of the machine.
pub struct ProcessTable {
    /// The processes, in the order they were created.
    pub processes: Vec<Process>,
    /// The identifier the next process gets.
    pub next_pid: u64,
}

impl ProcessTable {
    pub const fn new() -> Self {
        Self {
            processes: Vec::new(),
            next_pid: 1,
        }
    }

    /// The number of processes in the table.
    pub fn count(&self) -> usize {
        self.processes.len()
    }

    /// Take the identifier of the next process.
    pub fn allocate_pid(&mut self) -> ProcessId {
        let id = ProcessId(self.next_pid);
        self.next_pid += 1;
        id
    }

    /// Add a process to the table.
    pub fn spawn(&mut self, process: Process) {
        if self.processes.len() < MAX_PROCESSES {
            self.processes.push(process);
        }
    }

    /// The process with that identifier.
    pub fn get(&self, pid: ProcessId) -> Option<&Process> {
        self.processes.iter().find(|p| p.id == pid)
    }

    /// The process with that identifier, for writing.
    pub fn get_mut(&mut self, pid: ProcessId) -> Option<&mut Process> {
        self.processes.iter_mut().find(|p| p.id == pid)
    }

    /// Get a process by pid if it exists and is not reaped.
    pub fn find(&self, pid: ProcessId) -> Option<&Process> {
        self.processes.iter().find(|p| p.id == pid && p.state != ProcessState::Reaped)
    }

    /// Get mutable process if not reaped.
    pub fn find_mut(&mut self, pid: ProcessId) -> Option<&mut Process> {
        self.processes.iter_mut().find(|p| p.id == pid && p.state != ProcessState::Reaped)
    }

    /// Mark process as reaped.
    pub fn reap(&mut self, pid: ProcessId) -> bool {
        if let Some(process) = self.processes.iter_mut().find(|p| p.id == pid) {
            if process.state == ProcessState::Exited || process.state == ProcessState::Faulted {
                process.state = ProcessState::Reaped;
                return true;
            }
        }
        false
    }

    /// Get a process by pid if it exists and is not reaped.
    pub fn find(&self, pid: ProcessId) -> Option<&Process> {
        self.processes.iter().find(|p| p.id == pid && p.state != ProcessState::Reaped)
    }

    /// Get mutable process if not reaped.
    pub fn find_mut(&mut self, pid: ProcessId) -> Option<&mut Process> {
        self.processes.iter_mut().find(|p| p.id == pid && p.state != ProcessState::Reaped)
    }

    /// Mark process as reaped.
    pub fn reap(&mut self, pid: ProcessId) -> bool {
        if let Some(process) = self.processes.iter_mut().find(|p| p.id == pid) {
            if process.state == ProcessState::Exited || process.state == ProcessState::Faulted {
                process.state = ProcessState::Reaped;
                return true;
            }
        }
        false
    }
}

impl Default for ProcessTable {
    fn default() -> Self {
        Self::new()
    }
}

/// The process table of the machine.
static TABLE: Mutex<ProcessTable> = Mutex::new(ProcessTable::new());

/// The identifier of the process that runs at the moment.
static RUNNING: Mutex<Option<ProcessId>> = Mutex::new(None);

/// Register a program and return the identifier of its process.
pub fn register(name: &str, parent: Option<ProcessId>, entry: u64) -> ProcessId {
    register_with_identity(name, parent, entry, Process::kernel_identity())
}

/// Register a program with specific identity
pub fn register_with_identity(name: &str, parent: Option<ProcessId>, entry: u64, security: SecurityIdentity) -> ProcessId {
    let mut table = TABLE.lock();
    let id = table.allocate_pid();
    let process = Process {
        id,
        name: String::from(name),
        parent,
        security,
        state: ProcessState::Created,
        status: ExitStatus::Pending,
        address_space: crate::process::process::AddressSpace::new(0),
        thread: crate::process::process::UserThread {
            id: id.0,
            state: crate::process::process::UserThreadState::Runnable,
            stack_pointer: 0,
        },
        program: crate::process::process::ProgramDescriptor { entry },
        resources: crate::process::process::ResourceTable::empty(),
        interrupt_stack: 0,
        frames_owned: 0,
        syscalls: 0,
    };
    table.spawn(process);
    id
}

/// Mark a process as running.
pub fn set_running(pid: ProcessId) {
    let mut table = TABLE.lock();
    if let Some(process) = table.get_mut(pid) {
        process.set_running();
    }
    *RUNNING.lock() = Some(pid);
}

/// Record how a process ended.
pub fn finish(pid: ProcessId, status: ExitStatus) {
    let mut table = TABLE.lock();
    if let Some(process) = table.get_mut(pid) {
        process.finish(status);
    }
    let mut running = RUNNING.lock();
    if *running == Some(pid) {
        *running = None;
    }
}

/// Find a process if it exists and is not reaped.
pub fn find(pid: ProcessId) -> Option<Process> {
    let table = TABLE.lock();
    table.find(pid).cloned()
}

/// Get mutable access to process if not reaped.
pub fn find_mut(pid: ProcessId) -> Option<()> {
    let mut table = TABLE.lock();
    if table.find_mut(pid).is_some() {
        Some(())
    } else {
        None
    }
}

/// Mark process as reaped.
pub fn reap(pid: ProcessId) -> bool {
    let mut table = TABLE.lock();
    table.reap(pid)
}

/// The number of processes the table holds.
pub fn count() -> usize {
    TABLE.lock().count()
}

/// Mark a process as waiting.
pub fn set_waiting(pid: ProcessId) {
    let mut table = TABLE.lock();
    if let Some(process) = table.get_mut(pid) {
        process.state = ProcessState::Waiting;
    }
}

/// Wait for a child process to exit/fault and reap it.
pub fn wait(pid: Option<ProcessId>) -> Option<(ProcessId, ExitStatus)> {
    let mut table = TABLE.lock();
    if let Some(target_pid) = pid {
        if let Some(process) = table.processes.iter_mut().find(|p| p.id == target_pid) {
            if process.state == ProcessState::Exited || process.state == ProcessState::Faulted {
                let status = process.status;
                process.state = ProcessState::Reaped;
                // cleanup
                let handles: alloc::vec::Vec<u32> = process
                    .resources
                    .handles
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| process.resources.used[*i])
                    .map(|(_, &h)| h)
                    .collect();
                process.resources.clear();
                let _ = crate::storage::with_filesystem(|fs| {
                    for h in handles {
                        let _ = fs.close(crate::fs::FileHandle(h));
                    }
                });
                process.address_space.cleanup(0);
                return Some((target_pid, status));
            }
            if process.state != ProcessState::Reaped {
                process.state = ProcessState::Waiting;
            }
            return None;
        }
        return None;
    }
    for process in table.processes.iter_mut() {
        if process.state == ProcessState::Exited || process.state == ProcessState::Faulted {
            let status = process.status;
            let pid_res = process.id;
            process.state = ProcessState::Reaped;
            process.address_space.cleanup(0);
            process.resources.clear();
            return Some((pid_res, status));
        }
    }
    None
}

/// Clean up a terminated process
pub fn cleanup_process(pid: ProcessId) {
    let mut table = TABLE.lock();
    if let Some(process) = table.processes.iter_mut().find(|p| p.id == pid) {
        if process.state == ProcessState::Exited || process.state == ProcessState::Faulted {
            let handles: alloc::vec::Vec<u32> = process
                .resources
                .handles
                .iter()
                .enumerate()
                .filter(|(i, _)| process.resources.used[*i])
                .map(|(_, &h)| h)
                .collect();
            process.resources.clear();
            let _ = crate::storage::with_filesystem(|fs| {
                for h in handles {
                    let _ = fs.close(crate::fs::FileHandle(h));
                }
            })
            .ok();
            process.address_space.cleanup(0);
        }
    }
}

/// Report the table of the processes on the console.
pub fn report() {
    let table = TABLE.lock();
    unsafe {
        serial::write_str("Elvaris kernel: ps: pid state name\r\n");
        for process in table.processes.iter() {
            serial::write_str("Elvaris kernel: ps: ");
            serial::write_fmt(core::format_args!("{}", process.id.0));
            serial::write_str(" ");
            serial::write_str(process.state.name());
            serial::write_str(" ");
            serial::write_str(&process.name);
            serial::write_str(" ");
            serial::write_str(process.status.name());
            serial::write_str("\r\n");
        }
    }
}

/// The names of the processes, for the diagnostics.
pub fn names() -> Vec<String> {
    TABLE
        .lock()
        .processes
        .iter()
        .map(|p| p.name.clone())
        .collect()
}

/// The name of a program, without its path.
pub fn short_name(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.is_empty() {
        path.to_string()
    } else {
        name.to_string()
    }
}

pub mod address_space;
pub mod process;
