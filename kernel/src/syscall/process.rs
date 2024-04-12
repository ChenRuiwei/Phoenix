//! Syscall for processes operations.

use alloc::{string::String, vec::Vec};

use systype::{SysResult, SyscallResult};

use crate::{
    loader::{get_app_data, get_app_data_by_name},
    mm::{UserReadPtr, UserWritePtr},
    processor::hart::{current_task, current_trap_cx},
    task::{spawn_user_task, yield_now},
};

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct CloneFlags: u64 {
        /* 共享内存 */
        const VM = 0x0000100;
        /* 共享文件系统信息 */
        const FS = 0x0000200;
        /* 共享已打开的文件 */
        const FILES = 0x0000400;
        /* 共享信号处理句柄 */
        const SIGHAND = 0x00000800;
        /* 共享 parent (新旧 task 的 getppid 返回结果相同) */
        const PARENT = 0x00008000;
        /* 新旧 task 置于相同线程组 */
        const THREAD = 0x00010000;
        /* create a new TLS for the child */
        const SETTLS = 0x00080000;
        /* set the TID in the parent */
        const PARENT_SETTID = 0x00100000;
        /* clear the TID in the child */
        const CHILD_CLEARTID = 0x00200000;
        /* set the TID in the child */
        const CHILD_SETTID = 0x01000000;
        /* clear child signal handler */
        const CHILD_CLEAR_SIGHAND = 0x100000000;
    }
}

// TODO:
/// _exit() system call terminates only the calling thread, and actions such as
/// reparenting child processes or sending SIGCHLD to the parent process are
/// performed only if this is the last thread in the thread group.
pub fn sys_exit(exit_code: i32) -> SyscallResult {
    log::info!(
        "[sys_exit]: exit code {}, sepc {:#x}",
        exit_code,
        current_trap_cx().sepc
    );
    let tid = current_task().tid();
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

// TODO:
pub fn sys_wait4() -> SyscallResult {
    // The value status & 0xFF is returned to the parent process as the
    // process's exit status, and can be collected by the parent using one of
    // the wait(2) family of calls.
    // TODO: We should collect exit_code of child by & 0xFF
    todo!()
}

/// execve() executes the program referred to by pathname. This causes the
/// program that is currently being run by the calling process to be replaced
/// with a new program, with newly initialized stack, heap, and (initialized and
/// uninitialized) data segments.
///
/// If any of the threads in a thread group performs an execve(2), then all
/// threads other than the thread group leader are terminated, and the new
/// program is executed in the thread group leader.
pub fn sys_execve(
    path: UserReadPtr<u8>,
    argv: UserReadPtr<usize>,
    envp: UserReadPtr<usize>,
) -> SyscallResult {
    let task = current_task();
    let path_str = path.read_cstr(task)?;

    // TODO: path transefer to filename
    // let path = Path::from_string(path_str)?;
    // let path = if path.is_absolute() {
    //     path
    // } else {
    //     self.lproc.with_fsinfo(|f| f.cwd.append(&path))
    // };
    // let filename = path.last().clone();

    let read_2d_cstr = |mut ptr2d: UserReadPtr<usize>| -> SysResult<Vec<String>> {
        let ptr_vec: Vec<UserReadPtr<u8>> = ptr2d
            .read_cvec(task)?
            .into_iter()
            .map(UserReadPtr::from)
            .collect();
        let mut result = Vec::new();
        for ptr in ptr_vec {
            let str = ptr.read_cstr(task)?;
            result.push(str);
        }
        Ok(result)
    };

    let mut argv = read_2d_cstr(argv)?;
    let mut envp = read_2d_cstr(envp)?;

    log::info!(
        "[sys_execve]: path: {:?}, argv: {:?}, envp: {:?}",
        path_str,
        argv,
        envp
    );
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

    // TODO: read file data
    // let file = if filename.ends_with(".sh") {
    //     argv.insert(0, String::from("busybox"));
    //     argv.insert(1, String::from("sh"));
    //     fs::get_root_dir().lookup("busybox").await?
    // } else {
    //     fs::get_root_dir().resolve(&path).await?
    // };
    //

    // TODO: now we just load app data into kernel and read it
    task.do_execve(get_app_data_by_name(path_str.as_str()).unwrap(), argv, envp);
    Ok(0)
}
/// 功能：创建一个子进程；
/// 输入：
/// flags: 创建的标志，如SIGCHLD；
/// stack: 指定新进程的栈，可为0；
/// ptid: 父线程ID；
/// tls: TLS线程本地存储描述符；
/// ctid: 子线程ID；
/// 返回值：成功则返回子进程的线程ID，失败返回-1；
// TODO:
pub fn sys_clone(
    flags: usize,
    stack_ptr: usize,
    parent_tid_ptr: usize,
    tls_ptr: usize,
    chilren_tid_ptr: usize,
) -> SyscallResult {
    let flags = CloneFlags::from_bits(flags.try_into().unwrap()).unwrap();

    //     Since Linux 2.5.35, the flags mask must also include
    //               CLONE_SIGHAND if CLONE_THREAD is specified (and note that,
    //               since Linux 2.6.0, CLONE_SIGHAND also requires CLONE_VM to
    //               be included).
    let stack_begin = if stack_ptr != 0 {
        Some(stack_ptr.into())
    } else {
        None
    };
    let new_task = current_task().do_clone(flags, stack_begin);
    new_task.trap_context_mut().set_user_a0(0);
    let new_task_tid = new_task.pid();
    log::info!(
        "[sys_clone] clone a new process, pid {}, clone flags {:?}",
        new_task_tid,
        flags,
    );
    spawn_user_task(new_task);
    Ok(new_task_tid.into())
}

pub async fn sys_sched_yield() -> SyscallResult {
    yield_now().await;
    Ok(0)
}
