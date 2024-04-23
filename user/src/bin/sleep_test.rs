#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use time::{timespec::TimeSpec, timeval::TimeVal};
use user_lib::{exit, fork, gettimeofday, nanosleep, waitpid};

fn sleepy() {
    let time: usize = 1000;
    for i in 0..5 {
        let mut rem = TimeSpec::from_ms(0);
        nanosleep(&TimeSpec::from_ms(time), &mut rem);
        println!("sleep {} x {} msecs.", i + 1, time);
    }
    exit(0);
}

#[no_mangle]
pub fn main() -> i32 {
    let mut old_time_val = TimeVal::from_usec(0);
    gettimeofday(&mut old_time_val);
    let pid = fork();
    let mut exit_code: i32 = 0;
    if pid == 0 {
        sleepy();
    }
    assert!(waitpid(pid as usize, &mut exit_code) == pid && exit_code == 0);
    let mut new_time_val = TimeVal::from_usec(0);
    gettimeofday(&mut new_time_val);
    println!(
        "use {} usecs.",
        new_time_val.into_usec() - old_time_val.into_usec()
    );
    println!("sleep pass.");
    0
}
