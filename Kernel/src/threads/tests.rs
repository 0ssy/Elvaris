#![cfg(test)]

use super::*;
use super::context::ThreadContext;

#[test]
fn thread_context_size_is_valid() {
    assert!(core::mem::size_of::<ThreadContext>() > 0);
}

#[test]
fn thread_state_transitions() {
    let state = ThreadState::Created;
    assert_eq!(state, ThreadState::Created);
    let state = ThreadState::Runnable;
    assert_eq!(state, ThreadState::Runnable);
    let state = ThreadState::Running;
    assert_eq!(state, ThreadState::Running);
    let state = ThreadState::Blocked;
    assert_eq!(state, ThreadState::Blocked);
    let state = ThreadState::Terminated;
    assert_eq!(state, ThreadState::Terminated);
}

#[test]
fn thread_id_is_ordered() {
    let id1 = ThreadId(1);
    let id2 = ThreadId(2);
    assert!(id1 < id2);
    assert_ne!(id1, id2);
}
