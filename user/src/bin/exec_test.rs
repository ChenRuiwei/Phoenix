#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{execve, fork, println, wait, yield_};

#[no_mangle]
fn main() -> i32 {
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
        let mut wstatus: i32 = 0;
        let pid = wait(&mut wstatus);
        println!(
            "[initproc] Released a zombie process, pid={}, wstatus={:#x}",
            pid, wstatus,
        );
    }
    0
}