//! Syscall for processes operations.

use alloc::{string::String, vec::Vec};

use async_utils::yield_now;
use systype::{SysError, SysResult, SyscallResult};
use vfs::{sys_root_dentry, DISK_FS_NAME, FS_MANAGER};
use vfs_core::{InodeMode, OpenFlags, AT_FDCWD};

use crate::{
    loader::get_app_data_by_name,
    mm::{UserReadPtr, UserWritePtr},
    processor::hart::current_task,
    syscall::{at_helper, resolve_path},
    task::{spawn_user_task, PGid, Pid, TASK_MANAGER},
};

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    /// See in "bits/sched.h"
    pub struct CloneFlags: u64 {
        /// Set if VM shared between processes.
        const VM = 0x0000100;
        /// Set if fs info shared between processes.
        const FS = 0x0000200;
        /// Set if open files shared between processes.
        const FILES = 0x0000400;
        /// Set if signal handlers shared.
        const SIGHAND = 0x00000800;
        /// Set if we want to have the same parent as the cloner.
        const PARENT = 0x00008000;
        /// Set to add to same thread group.
        const THREAD = 0x00010000;
        /// Set TLS info.
        const SETTLS = 0x00080000;
        /// Store TID in userlevel buffer before MM copy.
        const PARENT_SETTID = 0x00100000;
        /// Register exit futex and memory location to clear.
        const CHILD_CLEARTID = 0x00200000;
        /// Store TID in userlevel buffer in the child.
        const CHILD_SETTID = 0x01000000;
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    /// See in "bits/waitflags.h"
    pub struct WaitOptions: i32 {
        /// Don't block waiting.
        const WNOHANG = 0x00000001;
        /// Report status of stopped children.
        const WUNTRACED = 0x00000002;
        /// Report continued child.
        const WCONTINUED = 0x00000008;
    }
}

/// _exit() system call terminates only the calling thread, and actions such as
/// reparenting child processes or sending SIGCHLD to the parent process are
/// performed only if this is the last thread in the thread group.
pub fn sys_exit(exit_code: i32) -> SyscallResult {
    let task = current_task();
    task.set_zombie();
    // non-leader thread are detached (see CLONE_THREAD flag in manual page clone.2)
    if task.is_leader() {
        task.set_exit_code(exit_code);
    }
    Ok(0)
}

/// This system call terminates all threads in the calling process's thread
/// group.
pub fn sys_exit_group(exit_code: i32) -> SyscallResult {
    let task = current_task();
    task.with_thread_group(|tg| {
        for t in tg.iter() {
            t.set_zombie();
        }
    });
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

/// NOTE: A thread can, and by default will, wait on children of other threads
/// in the same thread group.
// TODO: More options and process group support.
// PERF: use event bus to notify this task when child exits
pub async fn sys_wait4(
    pid: i32,
    wstatus: UserWritePtr<i32>,
    option: i32,
    _rusage: usize,
) -> SyscallResult {
    let task = current_task();
    let option = WaitOptions::from_bits_truncate(option);
    #[derive(Debug)]
    enum WaitFor {
        // wait for any child process in the specific process group
        PGid(PGid),
        // wait for any child process
        AnyChild,
        // wait for any child process in the same process group of the calling process
        AnyChildInGroup,
        // wait for the child process with the specific pid
        Pid(Pid),
    }
    let target = match pid {
        -1 => WaitFor::AnyChild,
        0 => WaitFor::AnyChildInGroup,
        p if p > 0 => WaitFor::Pid(p as Pid),
        p => WaitFor::PGid(p as PGid),
    };
    loop {
        // log::debug!("[sys_wait4]: finding zombie children for {target:?}");
        let children = task.children();
        if children.is_empty() {
            log::warn!("[sys_wait4] fail: no child");
            return Err(SysError::ECHILD);
        }
        let res_task = match target {
            WaitFor::AnyChild => children.values().find(|c| c.is_zombie()),
            WaitFor::Pid(pid) => {
                if let Some(child) = children.get(&pid) {
                    if child.is_zombie() {
                        task.time_stat()
                            .update_child_time(child.time_stat().user_system_time());
                        Some(child)
                    } else {
                        None
                    }
                } else {
                    log::warn!("[sys_wait4] fail: no child with pid {pid}");
                    return Err(SysError::ECHILD);
                }
            }
            WaitFor::PGid(_) => unimplemented!(),
            WaitFor::AnyChildInGroup => unimplemented!(),
        };
        if let Some(res_task) = res_task {
            if wstatus.not_null() {
                // wstatus stores signal in the lowest 8 bits and exit code in higher 8 bits
                // wstatus macros can be found in "bits/waitstatus.h"
                let status = (res_task.exit_code() & 0xff) << 8;
                log::trace!("[sys_wait4] wstatus: {:#x}", status);
                wstatus.write(&task, status)?;
            }
            task.remove_child(res_task.tid());
            TASK_MANAGER.remove(res_task);
            return Ok(res_task.pid());
        } else if option.contains(WaitOptions::WNOHANG) {
            return Ok(0);
        }
        yield_now().await;
    }
}

/// execve() executes the program referred to by pathname. This causes the
/// program that is currently being run by the calling process to be replaced
/// with a new program, with newly initialized stack, heap, and (initialized and
/// uninitialized) data segments.
///
/// If any of the threads in a thread group performs an execve(2), then all
/// threads other than the thread group leader are terminated, and the new
/// program is executed in the thread group leader.
pub async fn sys_execve(
    path: UserReadPtr<u8>,
    argv: UserReadPtr<usize>,
    envp: UserReadPtr<usize>,
) -> SyscallResult {
    let task = current_task();
    let path = path.read_cstr(&task)?;

    let read_2d_cstr = |ptr2d: UserReadPtr<usize>| -> SysResult<Vec<String>> {
        let ptr_vec: Vec<UserReadPtr<u8>> = ptr2d
            .read_cvec(&task)?
            .into_iter()
            .map(UserReadPtr::from)
            .collect();
        let mut result = Vec::new();
        for ptr in ptr_vec {
            let str = ptr.read_cstr(&task)?;
            result.push(str);
        }
        Ok(result)
    };

    let argv = read_2d_cstr(argv)?;
    let envp = read_2d_cstr(envp)?;

    log::info!("[sys_execve]: path: {path:?}, argv: {argv:?}, envp: {envp:?}",);
    log::debug!("[sys_execve]: pid: {:?}", task.tid());

    // TODO: should we add envp
    // Mankor: 不知道为什么要加，从 Oops 抄过来的
    // envp.push(String::from("LD_LIBRARY_PATH=."));
    // envp.push(String::from("SHELL=/busybox"));
    // envp.push(String::from("PWD=/"));
    // envp.push(String::from("USER=root"));
    // envp.push(String::from("MOTD_SHOWN=pam"));
    // envp.push(String::from("LANG=C.UTF-8"));
    // envp.push(String::from(
    //     "INVOCATION_ID=e9500a871cf044d9886a157f53826684",
    // ));
    // envp.push(String::from("TERM=vt220"));
    // envp.push(String::from("SHLVL=2"));
    // envp.push(String::from("JOURNAL_STREAM=8:9265"));
    // envp.push(String::from("OLDPWD=/root"));
    // envp.push(String::from("_=busybox"));
    // envp.push(String::from("LOGNAME=root"));
    // envp.push(String::from("HOME=/"));
    // envp.push(String::from("PATH=/"));

    let mut elf_data = Vec::new();
    let file = resolve_path(&path)?.open()?;
    file.read_all_from_start(&mut elf_data).await?;
    task.do_execve(&elf_data, argv, envp);
    Ok(0)
}

// TODO:
pub fn sys_clone(
    flags: usize,
    stack: usize,
    _parent_tid_ptr: usize,
    _tls_ptr: usize,
    _chilren_tid_ptr: usize,
) -> SyscallResult {
    let exit_signal = flags & 0xff;
    let flags = CloneFlags::from_bits(flags as u64 & !0xff).ok_or(SysError::EINVAL)?;

    log::info!("[sys_clone] flags {flags:?}");
    let stack = if stack != 0 { Some(stack.into()) } else { None };
    let new_task = current_task().do_clone(flags, stack);
    new_task.trap_context_mut().set_user_a0(0);
    let new_tid = new_task.tid();
    log::info!("[sys_clone] clone a new thread, tid {new_tid}, clone flags {flags:?}",);
    spawn_user_task(new_task);
    Ok(new_tid)
}

pub async fn sys_sched_yield() -> SyscallResult {
    yield_now().await;
    Ok(0)
}
