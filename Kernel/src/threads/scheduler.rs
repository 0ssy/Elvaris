//! Thread scheduler for cooperative multitasking.

use crate::serial;
use crate::threads::{Thread, ThreadId, ThreadState};
use crate::threads::context::ThreadContext;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use spin::Mutex;

extern crate alloc;

/// Global scheduler.
static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

/// Thread scheduler.
pub struct Scheduler {
    threads: Vec<Option<Thread>>,
    runnable: VecDeque<ThreadId>,
    current: Option<ThreadId>,
    next_id: u64,
}

impl Scheduler {
    const fn new() -> Self {
        Self {
            threads: Vec::new(),
            runnable: VecDeque::new(),
            current: None,
            next_id: 1,
        }
    }
    
    /// Create a new thread.
    pub fn create_thread(&mut self, entry: extern "C" fn(*mut u8) -> !, arg: *mut u8, name: Option<alloc::string::String>) -> ThreadId {
        let thread = Thread::new(entry, arg, name.clone());
        let id = thread.id;
        let tid = id.0 as usize;
        
        // Ensure threads vec is large enough
        if tid >= self.threads.len() {
            self.threads.resize(tid + 1, None);
        }
        
        self.threads[tid] = Some(thread);
        self.runnable.push_back(id);
        
        unsafe {
            serial::write_str("Elvaris kernel: thread: created id=");
            // Simple number output
            let mut n = tid as u64;
            if n == 0 { serial::write_str("0"); } else {
                let mut buf = [0u8; 20];
                let mut i = 0;
                while n > 0 {
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    i += 1;
                }
                while i > 0 {
                    i -= 1;
                    let c = [buf[i]];
                    serial::write_str(core::str::from_utf8_unchecked(&c));
                }
            }
            serial::write_str(" name=");
            if let Some(ref n) = name {
                serial::write_str(n);
            } else {
                serial::write_str("unnamed");
            }
            serial::write_newline();
        }
        
        id
    }
    
    /// Get current thread.
    pub fn current_thread_id(&self) -> Option<ThreadId> {
        self.current
    }
    
    /// Yield to next runnable thread (cooperative).
    pub fn yield_now(&mut self) {
        if self.runnable.is_empty() {
            return;
        }
        
        let current_id = self.current;
        
        // Move current thread back to runnable if it's running
        if let Some(cid) = current_id {
            let tid = cid.0 as usize;
            if tid < self.threads.len() {
                if let Some(ref mut thread) = self.threads[tid] {
                    if thread.state == ThreadState::Running {
                        thread.state = ThreadState::Runnable;
                        self.runnable.push_back(cid);
                    }
                }
            }
        }
        
        // Get next thread
        let next_id = match self.runnable.pop_front() {
            Some(id) => id,
            None => return,
        };
        
        let next_tid = next_id.0 as usize;
        let current_tid = current_id.map(|id| id.0 as usize);
        
        // Switch if different thread
        if current_tid != Some(next_tid) {
            self.current = Some(next_id);
            
            // Set states
            if let Some(ctid) = current_tid {
                if ctid < self.threads.len() {
                    if let Some(ref mut thread) = self.threads[ctid] {
                        thread.state = ThreadState::Runnable;
                    }
                }
            }
            if next_tid < self.threads.len() {
                if let Some(ref mut thread) = self.threads[next_tid] {
                    thread.state = ThreadState::Running;
                }
            }
            
            // Perform context switch if we have both contexts
            if let (Some(ctid), true) = (current_tid, next_tid < self.threads.len()) {
                if ctid < self.threads.len() {
                    // Get raw pointers to contexts
                    let current_ctx = if let Some(ref mut thread) = self.threads[ctid] {
                        &mut thread.context as *mut ThreadContext
                    } else {
                        return;
                    };
                    let next_ctx = if let Some(ref thread) = self.threads[next_tid] {
                        &thread.context as *const ThreadContext
                    } else {
                        return;
                    };
                    
                    unsafe {
                        serial::write_str("Elvaris kernel: thread: switch ");
                        // print ids
                        let mut print_id = |id: usize| {
                            let mut n = id as u64;
                            if n == 0 { serial::write_str("0"); } else {
                                let mut buf = [0u8; 20];
                                let mut i = 0;
                                while n > 0 {
                                    buf[i] = b'0' + (n % 10) as u8;
                                    n /= 10;
                                    i += 1;
                                }
                                while i > 0 {
                                    i -= 1;
                                    let c = [buf[i]];
                                    serial::write_str(core::str::from_utf8_unchecked(&c));
                                }
                            }
                        };
                        print_id(ctid);
                        serial::write_str(" -> ");
                        print_id(next_tid);
                        serial::write_newline();
                        
                        crate::threads::switch::switch_context(current_ctx, next_ctx);
                    }
                }
            } else if next_tid < self.threads.len() {
                // First switch to this thread
                self.current = Some(next_id);
                if let Some(ref mut thread) = self.threads[next_tid] {
                    thread.state = ThreadState::Running;
                }
            }
        }
    }
    
    /// Exit current thread.
    pub fn exit_current(&mut self) {
        if let Some(cid) = self.current {
            let tid = cid.0 as usize;
            if tid < self.threads.len() {
                if let Some(ref mut thread) = self.threads[tid] {
                    thread.state = ThreadState::Terminated;
                }
            }
            self.current = None;
        }
    }
}

/// Create a new thread.
pub fn create_thread(entry: extern "C" fn(*mut u8) -> !, arg: *mut u8, name: Option<alloc::string::String>) -> ThreadId {
    let mut sched = SCHEDULER.lock();
    sched.create_thread(entry, arg, name)
}

/// Yield current thread.
pub fn yield_now() {
    let mut sched = SCHEDULER.lock();
    sched.yield_now();
}

/// Exit current thread.
pub fn exit_current() {
    let mut sched = SCHEDULER.lock();
    sched.exit_current();
}
