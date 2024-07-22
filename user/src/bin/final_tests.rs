#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::ToString;

use user_lib::{execve, fork, wait, waitpid};

#[macro_use]
extern crate user_lib;

const TESTCASES: [&str; 9] = [
    "time-test",
    "busybox_testcode.sh",
    "lua_testcode.sh",
    "time-test",
    "libc-bench",
    "libctest_testcode.sh",
    "lmbench_testcode.sh",
    "iozone_testcode.sh",
    "unixbench_testcode.sh",
];

fn run_cmd(cmd: &str) {
    if fork() == 0 {
        execve(
            "/busybox\0",
            &[
                "busybox\0".as_ptr(),
                "sh\0".as_ptr(),
                "-c\0".as_ptr(),
                cmd.as_ptr(),
            ],
            &[
                "PATH=/:/bin\0".as_ptr(),
                "LD_LIBRARY_PATH=/:/lib:/lib/glibc/:/lib/musl\0".as_ptr(),
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
            let pid = fork();
            if pid == 0 {
                let testname = testcase.to_string() + "\0";
                if execve(
                    &testname,
                    &[testname.as_ptr(), core::ptr::null::<u8>()],
                    &[
                        "PATH=/:/bin\0".as_ptr(),
                        "LD_LIBRARY_PATH=/:/lib:/lib64/lp64d:/usr/lib:/usr/local/lib:\0".as_ptr(),
                        "TERM=screen\0".as_ptr(),
                        core::ptr::null::<u8>(),
                    ],
                ) != 0
                {
                    println!("Error when executing!");
                    return 0;
                }
            } else {
                let mut exit_code: i32 = 0;
                waitpid(pid as usize, &mut exit_code);
            }
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
