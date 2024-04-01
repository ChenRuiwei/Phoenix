#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(core_intrinsics)]
#![feature(let_chains)]
#![feature(trait_upcasting)]
#![feature(panic_info_message)]
#![feature(sync_unsafe_cell)]

use alloc::{boxed::Box, sync::Arc};

use arch::interrupts;
use config::mm::HART_START_ADDR;
use driver::{sbi, BLOCK_DEVICE, CHAR_DEVICE, KERNEL_PAGE_TABLE};
use mm::KERNEL_SPACE;

use crate::processor::hart;

extern crate alloc;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate driver;

mod boot;
mod loader;
mod mm;
mod panic;
mod processor;
mod syscall;
mod task;
mod trap;
mod utils;

use core::{
    arch::global_asm,
    hint::{self},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

global_asm!(include_str!("trampoline.S"));
global_asm!(include_str!("link_app.S"));

static FIRST_HART: AtomicBool = AtomicBool::new(true);
static INIT_FINISHED: AtomicBool = AtomicBool::new(false);

#[allow(unused)]
fn hart_start(hart_id: usize) {
    use crate::processor::HARTS;

    // only start two harts
    let mut has_another = false;
    let hart_num = unsafe { HARTS.len() };
    for i in 0..hart_num {
        if has_another {
            break;
        }
        if i == hart_id {
            continue;
        }
        let status = sbi::hart_start(i, HART_START_ADDR);
        println!("[kernel] start to wake up hart {}... status {}", i, status);
        if status == 0 {
            has_another = true;
        }
    }
}

/// the rust entry-point of os
#[no_mangle]
fn rust_main(hart_id: usize) {
    if FIRST_HART
        .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        // The first hart
        boot::clear_bss();

        boot::print_boot_message();

        hart::init(hart_id);
        utils::logging::init();

        println!(
            "[kernel] ---------- main hart {} started ---------- ",
            hart_id
        );

        mm::init();
        mm::remap_test();
        trap::init();
        loader::init();

        // task::spawn_kernel_task(async move {
        //     process::add_initproc();
        // });

        // barrier
        INIT_FINISHED.store(true, Ordering::SeqCst);

        #[cfg(feature = "smp")]
        hart_start(hart_id);
    } else {
        // The other harts
        hart::init(hart_id);

        // barrier
        while !INIT_FINISHED.load(Ordering::SeqCst) {
            hint::spin_loop()
        }

        println!(
            "[kernel] ---------- hart {} is starting... ----------",
            hart_id
        );

        trap::init();
        mm::activate_kernel_space();
        println!("[kernel] ---------- hart {} started ----------", hart_id);
    }

    println!(
        "[kernel] ---------- hart {} start to fetch task... ---------- ",
        hart_id
    );
    loop {
        executor::run_until_idle();
    }
}
