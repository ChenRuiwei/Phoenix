use alloc::sync::{Arc, Weak};

use config::process::INITPROC_PID;
use hashbrown::HashMap;
use spin::Lazy;
use sync::mutex::SpinNoIrqLock;

use super::task::Task;

pub static TASK_MANAGER: Lazy<TaskManager> = Lazy::new(|| TaskManager::new());

pub struct TaskManager(SpinNoIrqLock<HashMap<usize, Weak<Task>>>);

impl TaskManager {
    pub fn new() -> Self {
        Self(SpinNoIrqLock::new(HashMap::new()))
    }

    pub fn add_task(&self, pid: usize, task: &Arc<Task>) {
        self.0.lock().insert(pid, Arc::downgrade(task));
    }

    pub fn remove_task(&self, pid: usize) {
        self.0.lock().remove(&pid);
    }

    /// Get the init process
    pub fn init_proc(&self) -> Arc<Task> {
        self.find_task_by_pid(INITPROC_PID).unwrap()
    }

    pub fn find_task_by_pid(&self, pid: usize) -> Option<Arc<Task>> {
        match self.0.lock().get(&pid) {
            Some(task) => task.upgrade(),
            None => None,
        }
    }

    pub fn total_num(&self) -> usize {
        self.0.lock().len()
    }
}
