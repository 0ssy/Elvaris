//! Process types.

#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Created,
    Runnable,
    Running,
    Exited,
    Faulted,
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

#[derive(Debug, Clone, Copy)]
pub struct SecurityIdentity {
    pub uid: u64,
    pub gid: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct ResourceTable {
    pub count: usize,
}

impl ResourceTable {
    pub const fn empty() -> Self {
        Self { count: 0 }
    }
}

pub const MAX_OWNED_FRAMES: usize = 32;

#[derive(Debug, Clone, Copy)]
pub struct AddressSpace {
    pub pml4_phys: u64,
    pub owned_count: usize,
    pub user_pages: u64,
}

impl AddressSpace {
    pub const fn new(pml4_phys: u64) -> Self {
        Self {
            pml4_phys,
            owned_count: 0,
            user_pages: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProgramDescriptor {
    pub entry: u64,
}

pub const MAX_RESOURCES: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct Process {
    pub id: ProcessId,
    pub name: &'static str,
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
}

impl Process {
    pub const fn kernel_identity() -> SecurityIdentity {
        SecurityIdentity { uid: 0, gid: 0 }
    }
}
