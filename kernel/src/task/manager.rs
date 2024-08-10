use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
    vec::Vec,
};

use config::process::INIT_PROC_PID;
use hashbrown::HashMap;
use spin::Lazy;
use sync::mutex::SpinNoIrqLock;
use systype::SysResult;

use super::{task::Task, PGid, Tid};

pub static TASK_MANAGER: Lazy<TaskManager> = Lazy::new(TaskManager::new);

pub static PROCESS_GROUP_MANAGER: ProcessGroupManager = ProcessGroupManager::new();

/// Tid -> Task
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

    pub fn tasks(&self) -> Vec<Arc<Task>> {
        self.0
            .lock()
            .values()
            .map(|t| t.upgrade().unwrap())
            .collect()
    }

    pub fn for_each(&self, f: impl Fn(&Arc<Task>) -> SysResult<()>) -> SysResult<()> {
        for task in self.0.lock().values() {
            f(&task.upgrade().unwrap())?
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.0.lock().len()
    }
}

/// PGid -> Process group
// TODO: process group should be created by shell forking, but how do we
// recognize a shell? may be by sid, which will introduce session in extra.
pub struct ProcessGroupManager(SpinNoIrqLock<BTreeMap<PGid, Vec<Weak<Task>>>>);

impl ProcessGroupManager {
    pub const fn new() -> Self {
        Self(SpinNoIrqLock::new(BTreeMap::new()))
    }

    pub fn add_group(&self, group_leader: &Arc<Task>) {
        let pgid = group_leader.tid();
        group_leader.set_pgid(pgid);
        let mut group = Vec::new();
        group.push(Arc::downgrade(group_leader));
        self.0.lock().insert(pgid, group);
    }

    pub fn add_process(&self, pgid: PGid, process: &Arc<Task>) {
        if !process.is_leader() {
            log::warn!("[ProcessGroupManager::add_process] try adding task that is not a process");
            return;
        }
        process.set_pgid(pgid);
        let mut inner = self.0.lock();
        let vec = inner.get_mut(&pgid).unwrap();
        vec.push(Arc::downgrade(process));
    }

    pub fn get_group(&self, pgid: PGid) -> Option<Vec<Weak<Task>>> {
        self.0.lock().get(&pgid).cloned()
    }

    pub fn remove(&self, process: &Arc<Task>) {
        self.0
            .lock()
            .get_mut(&process.pgid())
            .unwrap()
            .retain(|task| task.upgrade().map_or(false, |t| !Arc::ptr_eq(process, &t)))
    }
}
