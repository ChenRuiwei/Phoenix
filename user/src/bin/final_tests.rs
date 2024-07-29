#![no_std]
#![no_main]
#![feature(generic_arg_infer)]

extern crate alloc;

use alloc::string::ToString;

use user_lib::{execve, fork, wait, waitpid};

#[macro_use]
extern crate user_lib;

const TESTCASES: [&str; 53] = [
    "time-test",
    "busybox_testcode.sh",
    "lua_testcode.sh",
    "netperf_testcode.sh",
    "libc-bench",
    "libctest_testcode.sh",
    "iozone_testcode.sh",
    "unixbench_testcode.sh",
    "lmbench_testcode.sh",
    "cyclictest_testcode.sh",
    "iperf_testcode.sh",
    "./test-ltp.sh ltp/testcases/bin/abs01",
    "./test-ltp.sh ltp/testcases/bin/read01",
    "./test-ltp.sh ltp/testcases/bin/write01",
    "./test-ltp.sh ltp/testcases/bin/chdir04",
    "./test-ltp.sh ltp/testcases/bin/chmod01",
    "./test-ltp.sh ltp/testcases/bin/chown01",
    "./test-ltp.sh ltp/testcases/bin/chown02",
    "./test-ltp.sh ltp/testcases/bin/clock_gettime02",
    "./test-ltp.sh ltp/testcases/bin/close01",
    "./test-ltp.sh ltp/testcases/bin/close02",
    "./test-ltp.sh ltp/testcases/bin/creat01",
    "./test-ltp.sh ltp/testcases/bin/creat03",
    "./test-ltp.sh ltp/testcases/bin/creat05",
    "./test-ltp.sh ltp/testcases/bin/dup01",
    "./test-ltp.sh ltp/testcases/bin/dup02",
    "./test-ltp.sh ltp/testcases/bin/dup03",
    "./test-ltp.sh ltp/testcases/bin/dup3_01",
    "./test-ltp.sh ltp/testcases/bin/dup3_02",
    "./test-ltp.sh ltp/testcases/bin/dup04",
    "./test-ltp.sh ltp/testcases/bin/dup06",
    "./test-ltp.sh ltp/testcases/bin/dup07",
    "./test-ltp.sh ltp/testcases/bin/dup201",
    "./test-ltp.sh ltp/testcases/bin/dup202",
    "./test-ltp.sh ltp/testcases/bin/dup203",
    "./test-ltp.sh ltp/testcases/bin/dup204",
    "./test-ltp.sh ltp/testcases/bin/dup205",
    "./test-ltp.sh ltp/testcases/bin/dup206",
    "./test-ltp.sh ltp/testcases/bin/dup207",
    "./test-ltp.sh ltp/testcases/bin/gettid02",
    "./test-ltp.sh ltp/testcases/bin/getuid01",
    "./test-ltp.sh ltp/testcases/bin/kill03",
    "./test-ltp.sh ltp/testcases/bin/kill06",
    "./test-ltp.sh ltp/testcases/bin/kill08",
    "./test-ltp.sh ltp/testcases/bin/open01",
    "./test-ltp.sh ltp/testcases/bin/open03",
    "./test-ltp.sh ltp/testcases/bin/open04",
    "./test-ltp.sh ltp/testcases/bin/open06",
    "./test-ltp.sh ltp/testcases/bin/page01",
    "./test-ltp.sh ltp/testcases/bin/page02",
    "./test-ltp.sh ltp/testcases/bin/pipe01",
    "./test-ltp.sh ltp/testcases/bin/pipe02",
    "./test-ltp.sh ltp/testcases/bin/pipe03",
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
