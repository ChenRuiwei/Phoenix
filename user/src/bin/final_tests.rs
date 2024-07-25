#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::ToString;

use user_lib::{execve, fork, wait, waitpid};

#[macro_use]
extern crate user_lib;

const TESTCASES: [&str; 11] = [
    "time-test",
    "busybox_testcode.sh",
    "lua_testcode.sh",
    "netperf_testcode.sh",
    "./test-ltp.sh ltp/testcases/bin/abs01",
    "./test-ltp.sh ltp/testcases/bin/read01",
    "libc-bench",
    "libctest_testcode.sh",
    "lmbench_testcode.sh",
    "iozone_testcode.sh",
    "unixbench_testcode.sh",
];

fn run_cmd(cmd: &str) {
    if fork() == 0 {
        execve(
            "busybox\0",
            &[
                "busybox\0".as_ptr(),
                "sh\0".as_ptr(),
                "-c\0".as_ptr(),
                cmd.as_ptr(),
                core::ptr::null::<u8>(),
            ],
            &[
                "PATH=/:/bin\0".as_ptr(),
                "LD_LIBRARY_PATH=/:/lib:/lib/glibc/:/lib/musl\0".as_ptr(),
                core::ptr::null::<u8>(),
            ],
        );
    } else {
        let mut result: i32 = 0;
        waitpid((-1isize) as usize, &mut result);
    }
}

#[no_mangle]
fn main() -> i32 {
    run_cmd("busybox touch sort.src\0");
    run_cmd("busybox cp /lib/dlopen_dso.so dlopen_dso.so\0");
    if fork() == 0 {
        for testcase in TESTCASES {
            let testname = testcase.to_string() + "\0";
            run_cmd(&testname);
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
