#![no_std]
#![no_main]

extern crate user_lib;

extern crate alloc;

use alloc::format;

use user_lib::{execve, fork, println, wait, waitpid};

fn run_cmd(cmd: &str) {
    if fork() == 0 {
        execve(
            "busybox",
            &["busybox", "sh", "-c", cmd],
            &[
                "PATH=/:/bin",
                "HOME=/home/crw",
                "LD_LIBRARY_PATH=/:/lib:/lib/glibc/:/lib/musl",
            ],
        );
    } else {
        let mut result: i32 = 0;
        waitpid((-1isize) as usize, &mut result);
    }
}

#[no_mangle]
fn main() -> i32 {
    run_cmd("busybox --install /bin");
    run_cmd("rm /bin/sh");
    run_cmd("ln -s /lib/glibc/ld-linux-riscv64-lp64d.so.1 /lib/ld-linux-riscv64-lp64d.so.1 ");

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
