#![no_std]
#![no_main]
#![feature(generic_arg_infer)]

extern crate alloc;

use alloc::string::ToString;

use user_lib::{execve, fork, wait, waitpid};

#[macro_use]
extern crate user_lib;

const TESTCASES: [&str; 9] = [
    "time-test",
    "busybox_testcode.sh",
    "lua_testcode.sh",
    "netperf_testcode.sh",
    "libc-bench",
    "libctest_testcode.sh",
    "iozone_testcode.sh",
    "unixbench_testcode.sh",
    "lmbench_testcode.sh",
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
    run_cmd("busybox touch sort.src");
    run_cmd("busybox cp /lib/dlopen_dso.so dlopen_dso.so");
    if fork() == 0 {
        for test in TESTCASES {
            run_cmd(&test);
        }
    } else {
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            if pid < 0 {
                break;
            }
        }
    }
    0
}
