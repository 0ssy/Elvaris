use alloc::string::String;
use alloc::vec::Vec;
use crate::security::{Credentials, ROOT_UID, ROOT_GID, USER_UID, USER_GID};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Created,
    Runnable,
    Running,
    Waiting,
    Exited,
    Faulted,
    Reaped,
}

impl ProcessState {
    pub fn name(self) -> &'static str {
        match self {
            ProcessState::Created => "created",
            ProcessState::Runnable => "runnable",
            ProcessState::Running => "running",
            ProcessState::Waiting => "waiting",
            ProcessState::Exited => "exited",
            ProcessState::Faulted => "faulted",
            ProcessState::Reaped => "reaped",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultInfo {
    pub vector: u8,
    pub error_code: u64,
    pub fault_address: u64,
    pub rip: u64,
    pub rsp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RejectInfo {
    pub reason: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitStatus {
    Pending,
    Exited(u64),
    Faulted(FaultInfo),
    Rejected(RejectInfo),
}

impl ExitStatus {
    pub fn name(self) -> &'static str {
        match self {
            ExitStatus::Pending => "pending",
            ExitStatus::Exited(_) => "exited",
            ExitStatus::Faulted(_) => "faulted",
            ExitStatus::Rejected(_) => "rejected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserThreadState {
    Runnable,
    Running,
    Terminated,
}

#[derive(Debug, Clone, Copy)]
pub struct UserThread {
    pub id: u64,
    pub state: UserThreadState,
    pub stack_pointer: u64,
}

#[derive(Debug, Clone)]
pub struct SecurityIdentity {
    pub uid: u64,
    pub gid: u64,
    pub credentials: Credentials,
}

pub const MAX_RESOURCES: usize = 8;

#[derive(Debug, Clone)]
pub struct ResourceTable {
    pub count: usize,
    pub handles: [u32; MAX_RESOURCES],
    pub used: [bool; MAX_RESOURCES],
}

impl ResourceTable {
    pub const fn empty() -> Self {
        Self {
            count: 0,
            handles: [0; MAX_RESOURCES],
            used: [false; MAX_RESOURCES],
        }
    }

    pub fn add_handle(&mut self, handle: u32) {
        for i in 0..MAX_RESOURCES {
            if !self.used[i] {
                self.handles[i] = handle;
                self.used[i] = true;
                self.count += 1;
                break;
            }
        }
    }

    pub fn clear(&mut self) {
        self.count = 0;
        self.used = [false; MAX_RESOURCES];
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AddressSpace {
    pub pml4_phys: u64,
    pub owned_count: usize,
    pub user_pages: u64,
    /// Track owned physical frames (for cleanup)
    pub owned_frames: [u64; MAX_OWNED_FRAMES],
    pub owned_frames_count: usize,
}

impl AddressSpace {
    pub const fn new(pml4_phys: u64) -> Self {
        Self {
            pml4_phys,
            owned_count: 0,
            user_pages: 0,
            owned_frames: [0; MAX_OWNED_FRAMES],
            owned_frames_count: 0,
        }
    }

    pub fn track_frame(&mut self, frame: u64) {
        if self.owned_frames_count < MAX_OWNED_FRAMES {
            self.owned_frames[self.owned_frames_count] = frame;
            self.owned_frames_count += 1;
        }
        self.owned_count += 1;
    }
}

#[derive(Debug, Clone)]
pub struct ProgramDescriptor {
    pub entry: u64,
}


#[derive(Debug, Clone)]
pub struct Process {
    pub id: ProcessId,
    pub name: String,
    pub parent: Option<ProcessId>,
    pub security: SecurityIdentity,
    pub state: ProcessState,
    pub status: ExitStatus,
    pub address_space: AddressSpace,
    pub thread: UserThread,
    pub program: ProgramDescriptor,
    pub resources: ResourceTable,
    pub interrupt_stack: u64,
    pub frames_owned: u64,
    pub syscalls: u64,
}

impl Process {
    pub fn set_runnable(&mut self) {
        self.state = ProcessState::Runnable;
    }

    pub fn set_running(&mut self) {
        self.state = ProcessState::Running;
    }

    pub fn finish(&mut self, status: ExitStatus) {
        self.status = status;
        self.state = match status {
            ExitStatus::Exited(_) => ProcessState::Exited,
            ExitStatus::Faulted(_) => ProcessState::Faulted,
            ExitStatus::Rejected(_) => ProcessState::Faulted,
            ExitStatus::Pending => ProcessState::Exited,
        };
    }

    pub const fn exit_code(&self) -> Option<u64> {
        match self.status {
            ExitStatus::Exited(code) => Some(code),
            _ => None,
        }
    }

    pub const fn kernel_identity() -> SecurityIdentity {
        SecurityIdentity { 
            uid: ROOT_UID, 
            gid: ROOT_GID,
            credentials: Credentials::root(),
        }
    }

    pub const fn user_identity() -> SecurityIdentity {
        SecurityIdentity { 
            uid: USER_UID, 
            gid: USER_GID,
            credentials: Credentials::user(),
        }
    }