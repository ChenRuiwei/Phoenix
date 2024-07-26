#![no_std]
#![no_main]

extern crate user_lib;

extern crate alloc;

use alloc::{borrow::ToOwned, ffi::CString, string::ToString};
use core::ffi::CStr;

use user_lib::{execve, fork, println, wait, waitpid};

const TESTCASES: [&str; 32] = [
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
    "mount",
    "munmap",
    "open",
    "openat",
    "pipe",
    "read",
    "sleep",
    "times",
    "umount",
    "uname",
    "unlink",
    "wait",
    "waitpid",
    "write",
    "yield",
];

#[no_mangle]
fn main() -> i32 {
    println!("******************************");
    println!("begin running preliminary tests");
    println!("******************************");
    if fork() == 0 {
        for testcase in TESTCASES {
            let pid = fork();
            if pid == 0 {
                if execve(testcase, &[testcase], &[]) != 0 {
                    println!("Error when executing!");
                    return 0;
                }
            } else {
                let mut exit_code: i32 = 0;
                waitpid(pid as usize, &mut exit_code);
            }
        }
        println!("******************************");
        println!("test finished");
        println!("******************************");
    } else {
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            if pid < 0 {
                break;
            }
            println!(
                "[initproc] Released a zombie process, pid={}, exit_code={}",
                pid, exit_code,
            );
        }
    }
    0
}
