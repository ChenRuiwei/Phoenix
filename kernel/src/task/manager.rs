use alloc:: sync::{Arc, Weak};
use config::process::INITPROC_PID;
use sync::mutex::SpinNoIrqLock;

use crate::stack_trace;

use super::task::Task;
use hashbrown::HashMap;
pub static TASK_MANAGER: TaskManager = TaskManager::new();
pub struct TaskManager(SpinNoIrqLock<HashMap<usize, Weak<Task>>>);

impl TaskManager {
    pub const fn new() -> Self {
        Self(SpinNoIrqLock::new(HashMap::new()))
    }

    pub fn add_task(&self, pid: usize, task:&Arc<Task>){
        stack_trace!();
        self.0.lock().insert(pid, Arc::downgrade(task));
    }

    pub fn remove_task(&self, pid: usize){
        stack_trace!();
        self.0.lock().remove(&pid);
    }

    /// Get the init process
    pub fn init_proc(&self) -> Arc<Task> {
        stack_trace!();
        self.find_task_by_pid(INITPROC_PID).unwrap()
    }

    pub fn find_task_by_pid(&self, pid: usize) -> Option<Arc<Task>> {
        stack_trace!();
        match self.0.lock().get(&pid) {
            Some(task) => task.upgrade(),
            None => None,
        }
    }

    pub fn total_num(&self) -> usize {
        stack_trace!();
        self.0.lock().len()
    }
}

