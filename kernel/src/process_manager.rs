use alloc::collections::LinkedList;
use common::process::Process;

pub enum State {
    Ready,
    Blocked,
    Running,
}

static PROCESSES: spin::Mutex<LinkedList<ManagedProcess>> = spin::Mutex::new(LinkedList::new());

pub struct ManagedProcess {
    process: Process,
    state: State,
}