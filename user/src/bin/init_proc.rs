#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{execve, fork, println, wait};

#[no_mangle]
fn main() -> i32 {
    if fork() == 0 {
        execve(
            "busybox\0",
            &[
                "busybox\0".as_ptr(),
                "sh\0".as_ptr(),
                core::ptr::null::<u8>(),
            ],
            &[
                "PATH=/:/bin\0".as_ptr(),
                "LD_LIBRARY_PATH=/:/lib:/lib/glibc/:/lib/musl\0".as_ptr(),
                "TERM=screen\0".as_ptr(),
                core::ptr::null::<u8>(),
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
