#![no_std]
#![no_main]

use time::timeval::TimeVal;
use user_lib::{gettimeofday, println};

extern crate user_lib;

extern crate alloc;

#[no_mangle]
fn main() -> i32 {
    println!("begin time test");
    let mut timeval = TimeVal::default();
    gettimeofday(&mut timeval);
    println!("timeval: {:?}", timeval);
    0
}
