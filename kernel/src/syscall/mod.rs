//! Implementation of syscalls

mod id;
mod signal;
use core::panic;

use id::*;
use log::error;
use systype::SyscallResult;

use crate::processor::{
    env::SumGuard,
    hart::{current_task, current_trap_cx},
};

macro_rules! sys_handler {
    ($handler: ident, $args: tt) => {
        {
            $handler$args
        }
    };
    ($handler: ident, $args: tt, $await: tt) => {
        {
            $handler$args.$await
        }
    };
}

/// Handle syscall exception with `syscall_id` and other arguments.
pub async fn syscall(syscall_id: usize, args: [usize; 6]) -> SyscallResult {
    match syscall_id {
        SYSCALL_EXIT => sys_handler!(sys_exit, (args[0] as i32)),
        SYSCALL_WRITE => sys_handler!(sys_write, (args[0], args[1], args[2]), await),
        _ => {
            error!("Unsupported syscall_id: {}", syscall_id);
            Ok(0)
        }
    }
}

pub async fn sys_write(fd: usize, buf: usize, len: usize) -> SyscallResult {
    assert!(fd == 1);
    let guard = SumGuard::new();
    let buf = unsafe { core::slice::from_raw_parts(buf as *const u8, len) };
    for b in buf {
        print!("{}", *b as char);
    }
    Ok(0)
}

/// _exit() system call terminates only the calling thread, and actions such as
/// reparenting child processes or sending SIGCHLD to the parent process are
/// performed only if this is the last thread in the thread group.
pub fn sys_exit(exit_code: i32) -> SyscallResult {
    log::info!(
        "[sys_exit]: exit code {}, sepc {:#x}",
        exit_code,
        current_trap_cx().sepc
    );
    let pid = current_task().pid();
    current_task().set_zombie();
    Ok(0)
}

// TODO:
/// This system call terminates all threads in the calling process's thread
/// group.
pub fn sys_exit_group(exit_code: i32) -> SyscallResult {
    let mut task = current_task();
    task.set_exit_code(exit_code);
    Ok(0)
}

/// getpid() returns the process ID (PID) of the calling process.
pub fn sys_getpid() -> SyscallResult {
    Ok(current_task().pid())
}

/// getppid() returns the process ID of the parent of the calling process. This
/// will be either the ID of the process that created this process using fork(),
/// or, if that process has already terminated, the ID of the process to which
/// this process has been reparented.
pub fn sys_getppid() -> SyscallResult {
    Ok(current_task().ppid())
}

/// TODO:
pub fn sys_wait4() -> SyscallResult {
    // The value status & 0xFF is returned to the parent process as the
    // process's exit status, and can be collected by the parent using one of
    // the wait(2) family of calls.
    // TODO: We should collect exit_code of child by & 0xFF
    todo!()
}
