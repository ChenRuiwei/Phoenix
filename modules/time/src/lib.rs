#![no_std]
#![no_main]

use core::time::Duration;

pub mod stat;
pub mod timespec;
pub mod timeval;
pub mod tms;

// clockid
pub const SUPPORT_CLOCK: usize = 2;
/// 一个可设置的系统级实时时钟，用于测量真实（即墙上时钟）时间
pub const CLOCK_REALTIME: usize = 0;
/// 一个不可设置的系统级时钟，代表自某个未指定的过去时间点以来的单调时间
pub const CLOCK_MONOTONIC: usize = 1;
/// 用于测量调用进程消耗的CPU时间
pub const CLOCK_PROCESS_CPUTIME_ID: usize = 2;
/// 用于测量调用线程消耗的CPU时间
pub const CLOCK_THREAD_CPUTIME_ID: usize = 3;

pub static mut CLOCK_DEVIATION: [Duration; SUPPORT_CLOCK] = [Duration::ZERO; SUPPORT_CLOCK];
