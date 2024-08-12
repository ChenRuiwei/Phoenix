#![no_std]
#![no_main]

extern crate user_lib;

extern crate alloc;

use alloc::format;

use user_lib::{execve, fork, println, wait, waitpid};

const BUSYBOX_CMDS: &[&str] = &[
    "ls", "cp", "mv", "rm", "mkdir", "rmdir", "ln", "cat", "echo", "grep", "find", "tar", "awk",
    "sed", "kill", "df", "du", "uname", "ping", "ip",
];

fn run_cmd(cmd: &str) {
    if fork() == 0 {
        execve(
            "busybox",
            &["busybox", "sh", "-c", cmd],
            &[
                "PATH=/:/bin",
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
    for cmd in BUSYBOX_CMDS {
        run_cmd(&format!("busybox ln /busybox /bin/{}", cmd));
    }
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
