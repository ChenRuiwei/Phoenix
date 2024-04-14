use alloc::sync::{Arc, Weak};

use config::process::INITPROC_PID;
use hashbrown::HashMap;
use spin::Lazy;
use sync::mutex::SpinNoIrqLock;

use super::{task::Task, Tid};

pub static TASK_MANAGER: Lazy<TaskManager> = Lazy::new(TaskManager::new);

pub struct TaskManager(SpinNoIrqLock<HashMap<Tid, Weak<Task>>>);

impl TaskManager {
    pub fn new() -> Self {
        Self(SpinNoIrqLock::new(HashMap::new()))
    }

    pub fn add(&self, task: &Arc<Task>) {
        self.0.lock().insert(task.tid(), Arc::downgrade(task));
    }

    pub fn remove(&self, task: &Arc<Task>) {
        self.0.lock().remove(&task.tid());
    }

    /// Get the init process
    pub fn init_proc(&self) -> Arc<Task> {
        self.get(INITPROC_PID).unwrap()
    }

    pub fn get(&self, tid: Tid) -> Option<Arc<Task>> {
        match self.0.lock().get(&tid) {
            Some(task) => task.upgrade(),
            None => None,
        }
    }

    pub fn total_num(&self) -> usize {
        self.0.lock().len()
    }
}
