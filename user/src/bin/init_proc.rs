#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{execve, fork, println, wait};

#[no_mangle]
fn main() -> i32 {
    if fork() == 0 {
        execve(
            "busybox",
            &["busybox", "sh"],
            &[
                "PATH=/:/bin",
                "LD_LIBRARY_PATH=/:/lib:/lib/glibc/:/lib/musl",
                "TERM=screen",
            ],
        );
    } else {
        loop {
            let mut wstatus: i32 = 0;
            let pid = wait(&mut wstatus);
            if pid < 0 {
                break;
            }
            println!(
                "[initproc] release a zombie process, pid={}, wstatus={}",
                pid, wstatus,
            );
        }
    }
    0
}
