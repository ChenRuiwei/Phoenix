//! Syscall for processes operations.

use alloc::{string::String, vec::Vec};

use systype::{SysResult, SyscallResult};

use crate::{
    loader::{get_app_data, get_app_data_by_name},
    mm::{UserReadPtr, UserWritePtr},
    processor::hart::{current_task, current_trap_cx},
};

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
    let pid = current_task().tid();
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
/// All threads other than the calling thread are destroyed during an execve().
pub fn sys_execve(path: usize, mut argv: usize, mut envp: usize) -> SyscallResult {
    let task = current_task();
    let path_str = UserReadPtr::<u8>::from(path).read_cstr(task)?;

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

    let argv = UserReadPtr::from(argv);
    let envp = UserReadPtr::from(envp);

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
