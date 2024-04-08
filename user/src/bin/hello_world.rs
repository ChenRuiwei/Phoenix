#![no_std]
#![no_main]

extern crate user_lib;

use alloc::string::String;

use user_lib::println;

#[no_mangle]
fn main() {
    println!("hello world");
}
