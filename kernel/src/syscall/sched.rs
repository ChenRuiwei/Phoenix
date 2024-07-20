use core::intrinsics::size_of;

use systype::{SysError, SyscallResult};

use super::Syscall;
use crate::{
    mm::{UserReadPtr, UserWritePtr},
    task::{resource::CpuMask, TASK_MANAGER},
};

impl Syscall<'_> {
    pub fn sys_sched_setscheduler(&self) -> SyscallResult {
        log::warn!("[sys_sched_setscheduler] unimplemented");
        Ok(0)
    }

    pub fn sys_sched_getscheduler(&self) -> SyscallResult {
        log::warn!("[sys_sched_getscheduler] unimplemented");
        Ok(0)
    }

    pub fn sys_sched_getparam(&self) -> SyscallResult {
        log::warn!("[sys_sched_getparam] unimplemented");
        Ok(0)
    }

    pub fn sys_sched_setaffinity(
        &self,
        pid: usize,
        cpusetsize: usize,
        mask: UserReadPtr<CpuMask>,
    ) -> SyscallResult {
        log::warn!("[sys_sched_setaffinity]");
        if cpusetsize < size_of::<CpuMask>() {
            return Err(SysError::EINVAL);
        }
        if let Some(task) = TASK_MANAGER.get(pid) {
            if !task.is_leader() {
                return Err(SysError::ESRCH);
            }
            let mask = mask.read(&self.task)?;
            *task.cpus_allowed() = mask;
        } else {
            return Err(SysError::ESRCH);
        }
        Ok(0)
    }

    pub fn sys_sched_getaffinity(
        &self,
        pid: usize,
        cpusetsize: usize,
        mask: UserWritePtr<CpuMask>,
    ) -> SyscallResult {
        log::warn!("[sys_sched_getaffinity]");
        if cpusetsize < size_of::<CpuMask>() {
            return Err(SysError::EINVAL);
        }
        if let Some(task) = TASK_MANAGER.get(pid) {
            if !task.is_leader() {
                return Err(SysError::ESRCH);
            }
            mask.write(&self.task, *task.cpus_allowed())?;
        } else {
            return Err(SysError::ESRCH);
        }
        Ok(0)
    }
}
