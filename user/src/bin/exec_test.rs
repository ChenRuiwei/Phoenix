#![no_std]
#![no_main]

extern crate user_lib;

extern crate alloc;

use user_lib::{execve, fork, println, wait};

#[no_mangle]
fn main() -> i32 {
    println!("begin exec_test");
    let tests = [
        "brk",
        "chdir",
        "clone",
        "close",
        "dup",
        "dup2",
        "execve",
        "exit",
        "fork",
        "fstat",
        "getcwd",
        "getdents",
        "getpid",
        "getppid",
        "gettimeofday",
        "mkdir_",
        "mmap",
        "mnt",
        "mount",
        "munmap",
        "open",
        "openat",
        "pipe",
        "read",
        "run-all.sh",
        "sleep",
        "test_echo",
        "text.txt",
        "times",
        "umount",
        "uname",
        "unlink",
        "wait",
        "waitpid",
        "write",
        "yield",
    ];
    if fork() == 0 {
        execve(
            "hello_world\0",
            &[
                "busybox\0".as_ptr(),
                "sh\0".as_ptr(),
                core::ptr::null::<u8>(),
            ],
            &[
                "PATH=/:/bin:/sbin:/usr/bin:/usr/local/bin:/usr/local/sbin:\0".as_ptr(),
                "LD_LIBRARY_PATH=/:/lib:/lib64/lp64d:/usr/lib:/usr/local/lib:\0".as_ptr(),
                "TERM=screen\0".as_ptr(),
                core::ptr::null::<u8>(),
            ],
        );
    } else {
        heap_test();
        let mut wstatus: i32 = 0;
        let pid = wait(&mut wstatus);
        println!(
            "[initproc] Released a zombie process, pid={}, wstatus={:#x}",
            pid, wstatus,
        );
    }
    0
}

fn heap_test() {
    use alloc::{boxed::Box, vec::Vec};
    let a = Box::new(5);
    assert_eq!(*a, 5);
    drop(a);
    let mut v: Vec<usize> = Vec::new();
    for i in 0..500 {
        v.push(i);
    }
    for (i, val) in v.iter().take(500).enumerate() {
        assert_eq!(*val, i);
    }
    drop(v);
    println!("heap_test passed");
}
