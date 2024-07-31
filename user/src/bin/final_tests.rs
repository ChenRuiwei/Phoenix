#![no_std]
#![no_main]
#![feature(generic_arg_infer)]

extern crate alloc;

use user_lib::{execve, fork, wait, waitpid};

#[macro_use]
extern crate user_lib;

const TESTCASES: [&str; 130] = [
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
    "./test-ltp.sh ltp/testcases/bin/chdir04",
    "./test-ltp.sh ltp/testcases/bin/chmod01",
    "./test-ltp.sh ltp/testcases/bin/chown01",
    "./test-ltp.sh ltp/testcases/bin/chown02",
    "./test-ltp.sh ltp/testcases/bin/clock_gettime02",
    "./test-ltp.sh ltp/testcases/bin/close01",
    "./test-ltp.sh ltp/testcases/bin/close02",
    "./test-ltp.sh ltp/testcases/bin/clone04",
    "./test-ltp.sh ltp/testcases/bin/clone302",
    "./test-ltp.sh ltp/testcases/bin/confstr01",
    "./test-ltp.sh ltp/testcases/bin/creat01",
    "./test-ltp.sh ltp/testcases/bin/creat03",
    "./test-ltp.sh ltp/testcases/bin/creat05",
    "./test-ltp.sh ltp/testcases/bin/diotest1",
    "./test-ltp.sh ltp/testcases/bin/diotest3",
    "./test-ltp.sh ltp/testcases/bin/diotest6",
    "./test-ltp.sh ltp/testcases/bin/dirty",
    "./test-ltp.sh ltp/testcases/bin/dup01",
    "./test-ltp.sh ltp/testcases/bin/dup02",
    "./test-ltp.sh ltp/testcases/bin/dup03",
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
    "./test-ltp.sh ltp/testcases/bin/dup3_01",
    "./test-ltp.sh ltp/testcases/bin/dup3_02",
    "./test-ltp.sh ltp/testcases/bin/epoll_create1_02",
    "./test-ltp.sh ltp/testcases/bin/epoll_ctl01",
    "./test-ltp.sh ltp/testcases/bin/epoll_ctl02",
    "./test-ltp.sh ltp/testcases/bin/epoll_ctl04",
    "./test-ltp.sh ltp/testcases/bin/epoll_ctl05",
    "./test-ltp.sh ltp/testcases/bin/epoll_wait01",
    "./test-ltp.sh ltp/testcases/bin/epoll_wait03",
    "./test-ltp.sh ltp/testcases/bin/epoll_wait04",
    "./test-ltp.sh ltp/testcases/bin/epoll_wait07",
    "./test-ltp.sh ltp/testcases/bin/exit02",
    "./test-ltp.sh ltp/testcases/bin/faccessat01",
    "./test-ltp.sh ltp/testcases/bin/faccessat02",
    "./test-ltp.sh ltp/testcases/bin/fchdir01",
    "./test-ltp.sh ltp/testcases/bin/fchdir02",
    "./test-ltp.sh ltp/testcases/bin/fchmod01",
    "./test-ltp.sh ltp/testcases/bin/fchmodat01",
    "./test-ltp.sh ltp/testcases/bin/fchmodat02",
    "./test-ltp.sh ltp/testcases/bin/fchown05",
    "./test-ltp.sh ltp/testcases/bin/fchown01",
    "./test-ltp.sh ltp/testcases/bin/fchown02",
    "./test-ltp.sh ltp/testcases/bin/fcntl02",
    "./test-ltp.sh ltp/testcases/bin/fcntl02_64",
    "./test-ltp.sh ltp/testcases/bin/fcntl03",
    "./test-ltp.sh ltp/testcases/bin/fcntl03_64",
    "./test-ltp.sh ltp/testcases/bin/fcntl04",
    "./test-ltp.sh ltp/testcases/bin/fcntl04_64",
    "./test-ltp.sh ltp/testcases/bin/fcntl05",
    "./test-ltp.sh ltp/testcases/bin/fcntl05_64",
    "./test-ltp.sh ltp/testcases/bin/fcntl08",
    "./test-ltp.sh ltp/testcases/bin/fcntl08_64",
    "./test-ltp.sh ltp/testcases/bin/fcntl13_64",
    "./test-ltp.sh ltp/testcases/bin/fcntl27",
    "./test-ltp.sh ltp/testcases/bin/fcntl29",
    "./test-ltp.sh ltp/testcases/bin/fcntl29_64",
    "./test-ltp.sh ltp/testcases/bin/fdatasync02",
    "./test-ltp.sh ltp/testcases/bin/fgetxattr03",
    "./test-ltp.sh ltp/testcases/bin/flistxattr01",
    "./test-ltp.sh ltp/testcases/bin/flistxattr02",
    "./test-ltp.sh ltp/testcases/bin/flistxattr03",
    "./test-ltp.sh ltp/testcases/bin/flock01",
    "./test-ltp.sh ltp/testcases/bin/flock04",
    "./test-ltp.sh ltp/testcases/bin/flock06",
    "./test-ltp.sh ltp/testcases/bin/fork01",
    "./test-ltp.sh ltp/testcases/bin/fork03",
    "./test-ltp.sh ltp/testcases/bin/fork05",
    "./test-ltp.sh ltp/testcases/bin/fork07",
    "./test-ltp.sh ltp/testcases/bin/fork08",
    "./test-ltp.sh ltp/testcases/bin/fork09",
    "./test-ltp.sh ltp/testcases/bin/fork10",
    "./test-ltp.sh ltp/testcases/bin/fork_procs",
    "./test-ltp.sh ltp/testcases/bin/fpathconf01",
    "./test-ltp.sh ltp/testcases/bin/fptest01",
    "./test-ltp.sh ltp/testcases/bin/fptest02",
    "./test-ltp.sh ltp/testcases/bin/fs_perms",
    "./test-ltp.sh ltp/testcases/bin/fstat02",
    "./test-ltp.sh ltp/testcases/bin/fstat02_64",
    "./test-ltp.sh ltp/testcases/bin/futex_wait01",
    "./test-ltp.sh ltp/testcases/bin/futex_wait03",
    "./test-ltp.sh ltp/testcases/bin/futex_wait04",
    "./test-ltp.sh ltp/testcases/bin/futex_wake01",
    "./test-ltp.sh ltp/testcases/bin/gettid02",
    "./test-ltp.sh ltp/testcases/bin/getuid01",
    "./test-ltp.sh ltp/testcases/bin/kill03",
    "./test-ltp.sh ltp/testcases/bin/kill06",
    "./test-ltp.sh ltp/testcases/bin/kill08",
    "./test-ltp.sh ltp/testcases/bin/open01",
    "./test-ltp.sh ltp/testcases/bin/open03",
    "./test-ltp.sh ltp/testcases/bin/open04",
    "./test-ltp.sh ltp/testcases/bin/open06",
    "./test-ltp.sh ltp/testcases/bin/pipe01",
    "./test-ltp.sh ltp/testcases/bin/pipe02",
    "./test-ltp.sh ltp/testcases/bin/pipe03",
    "./test-ltp.sh ltp/testcases/bin/read01",
    "./test-ltp.sh ltp/testcases/bin/write01",
    "./test-ltp.sh ltp/testcases/bin/accept01",
    "./test-ltp.sh ltp/testcases/bin/alarm02",
    "./test-ltp.sh ltp/testcases/bin/alarm03",
    "./test-ltp.sh ltp/testcases/bin/alarm05",
    "./test-ltp.sh ltp/testcases/bin/alarm06",
    "./test-ltp.sh ltp/testcases/bin/alarm07",
    "./test-ltp.sh ltp/testcases/bin/atof01",
    "./test-ltp.sh ltp/testcases/bin/chown05",
    "./test-ltp.sh ltp/testcases/bin/chroot03",
    "./test-ltp.sh ltp/testcases/bin/clock_getres01",
    "./test-ltp.sh ltp/testcases/bin/clock_nanosleep04",
    "./test-ltp.sh ltp/testcases/bin/clone01",
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
