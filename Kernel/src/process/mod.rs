//! Process table and run loop.

#![allow(dead_code)]

use crate::process::process::{Process, ProcessId, ProcessState};

pub const MAX_PROCESSES: usize = 16;

pub struct ProcessTable {
    pub processes: [Option<Process>; MAX_PROCESSES],
    pub next_pid: u64,
    pub run_queue: [ProcessId; MAX_PROCESSES],
    pub run_queue_head: usize,
    pub run_queue_len: usize,
    pub count: usize,
}

impl ProcessTable {
    pub const fn new() -> Self {
        Self {
            processes: [None; MAX_PROCESSES],
            next_pid: 1,
            run_queue: [ProcessId(0); MAX_PROCESSES],
            run_queue_head: 0,
            run_queue_len: 0,
            count: 0,
        }
    }

    pub fn allocate_pid(&mut self) -> ProcessId {
        let id = ProcessId(self.next_pid);
        self.next_pid += 1;
        id
    }

    pub fn spawn(&mut self, process: Process) {
        if self.count < MAX_PROCESSES {
            self.processes[self.count] = Some(process);
            self.run_queue[self.run_queue_len] = process.id;
            self.run_queue_len += 1;
            self.count += 1;
        }
    }

    pub fn get(&self, pid: ProcessId) -> Option<&Process> {
        for p in self.processes.iter().flatten() {
            if p.id.0 == pid.0 {
                return Some(p);
            }
        }
        None
    }

    pub fn report(&self) {
        let _ = self.count;
    }
}
pub mod process;
pub mod address_space;
