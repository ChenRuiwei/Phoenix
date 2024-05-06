use alloc::sync::{Arc, Weak};

use config::process::INIT_PROC_PID;
use hashbrown::HashMap;
use spin::Lazy;
use sync::mutex::SpinNoIrqLock;
use systype::SysResult;

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

    pub fn remove(&self, tid: Tid) {
        self.0.lock().remove(&tid);
    }

    /// Get the init process.
    pub fn init_proc(&self) -> Arc<Task> {
        self.get(INIT_PROC_PID).unwrap()
    }

    pub fn get(&self, tid: Tid) -> Option<Arc<Task>> {
        match self.0.lock().get(&tid) {
            Some(task) => task.upgrade(),
            None => None,
        }
    }

    pub fn for_each(&self, f: impl Fn(&Arc<Task>) -> SysResult<()>) -> SysResult<()> {
        for task in self.0.lock().values() {
            f(&task.upgrade().unwrap())?
        }
        Ok(())
    }

    pub fn total_num(&self) -> usize {
        self.0.lock().len()
    }
}
