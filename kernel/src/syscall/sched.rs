use core::intrinsics::size_of;

use systype::{SysError, SyscallResult};

use crate::{
    mm::{UserReadPtr, UserWritePtr},
    processor::hart::current_task,
    task::{resource::CpuMask, TASK_MANAGER},
};

pub fn sys_sched_setscheduler() -> SyscallResult {
    log::warn!("[sys_sched_setscheduler] unimplemented");
    Ok(0)
}

pub fn sys_sched_getscheduler() -> SyscallResult {
    log::warn!("[sys_sched_getscheduler] unimplemented");
    Ok(0)
}

pub fn sys_sched_getparam() -> SyscallResult {
    log::warn!("[sys_sched_getparam] unimplemented");
    Ok(0)
}

pub fn sys_sched_setaffinity(
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
        let mask = mask.read(&current_task())?;
        *task.cpus_allowed() = mask;
    } else {
        return Err(SysError::ESRCH);
    }
    Ok(0)
}

pub fn sys_sched_getaffinity(
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
        mask.write(&current_task(), *task.cpus_allowed())?;
    } else {
        return Err(SysError::ESRCH);
    }
    Ok(0)
}
