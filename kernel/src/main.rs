#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(core_intrinsics)]
#![feature(let_chains)]
#![feature(trait_upcasting)]
#![feature(panic_info_message)]
#![feature(const_trait_impl)]
#![feature(effects)]
#![feature(sync_unsafe_cell)]
#![feature(stdsimd)]
#![feature(riscv_ext_intrinsics)]
#![feature(map_try_insert)]
#![feature(byte_slice_trim_ascii)]
#![feature(new_uninit)]
#![allow(clippy::mut_from_ref)]

mod boot;
mod impls;
mod ipc;
mod loader;
mod mm;
mod net;
mod panic;
mod processor;
mod syscall;
mod task;
mod trap;
mod utils;
use core::{
    arch::global_asm,
    sync::atomic::{AtomicBool, Ordering},
};

use driver::BLOCK_DEVICE;
use executor::task_len;
use timer::timelimited_task::ksleep_s;

use crate::{processor::hart, task::TASK_MANAGER};

extern crate alloc;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate driver;

#[macro_use]
extern crate logging;

global_asm!(include_str!("trampoline.asm"));
global_asm!(include_str!("link_app.asm"));

static FIRST_HART: AtomicBool = AtomicBool::new(true);

#[no_mangle]
fn rust_main(hart_id: usize, dtb_addr: usize) {
    if FIRST_HART
        .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        boot::clear_bss();
        boot::print_banner();

        hart::init(hart_id);
        logging::init();

        println!("[kernel] ---------- main hart {hart_id} started ---------- ");
        config::mm::set_dtb_addr(dtb_addr);

        mm::init();
        trap::init();
        driver::init();
        vfs::init();
        loader::init();
        task::spawn_kernel_task(async move {
            task::spawn_init_proc();
        });

        #[cfg(feature = "smp")]
        boot::start_harts(hart_id);
    } else {
        hart::init(hart_id);
        trap::init();
        unsafe { mm::switch_kernel_page_table() };
    }

    unsafe {
        arch::interrupts::enable_timer_interrupt();
        arch::time::set_next_timer_irq()
    };

    println!("[kernel] ---------- hart {hart_id} start to fetch task... ---------- ");
    let mut try_count = 0usize;
    loop {
        let tasks = executor::run_until_idle();
        if tasks == 0 {
            try_count += 1;
        } else {
            try_count = 0;
        }
        if try_count >= 0x10000000 {
            panic!("no tasks")
        }
    }
}
