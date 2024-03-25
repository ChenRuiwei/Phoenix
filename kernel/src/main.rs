#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(core_intrinsics)]
#![feature(let_chains)]
#![feature(trait_upcasting)]
#![feature(panic_info_message)]

use alloc::{boxed::Box, sync::Arc};

use arch::interrupts;
use config::mm::HART_START_ADDR;
use driver::{
    plic::initplic,
    qemu::{self, virtio_blk::VirtIOBlock},
    sbi, BLOCK_DEVICE, CHAR_DEVICE, KERNEL_PAGE_TABLE,
};
use mm::KERNEL_SPACE;

use crate::{
    fs::TTY,
    process::{thread, PROCESS_MANAGER},
    processor::hart,
    timer::{timeout_task::ksleep, POLL_QUEUE},
};

extern crate alloc;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate driver;

mod boot;
mod fs;
mod futex;
mod loader;
mod mm;
mod net;
mod panic;
mod process;
mod processor;
mod signal;
mod syscall;
mod timer;
mod trap;
#[macro_use]
mod utils;

use core::{
    arch::{global_asm},
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
        driver_init();
        loader::init();
        fs::init();
        timer::init();
        net::config::init();

        thread::spawn_kernel_thread(async move {
            process::add_initproc();
        });

        #[cfg(not(feature = "submit"))]
        thread::spawn_kernel_thread(async move {
            loop {
                log::info!("[daemon] process cnt {}", PROCESS_MANAGER.total_num());
                ksleep(Duration::from_secs(3)).await;
            }
        });

        #[cfg(not(feature = "submit"))]
        thread::spawn_kernel_thread(async move {
            loop {
                POLL_QUEUE.poll();
                ksleep(Duration::from_millis(30)).await;
            }
        });

        // barrier
        INIT_FINISHED.store(true, Ordering::SeqCst);

        #[cfg(feature = "multi_hart")]
        hart_start(hart_id);

        arch::interrupts::enable_timer_interrupt();
        timer::set_next_trigger();
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

        arch::interrupts::enable_timer_interrupt();
        timer::set_next_trigger();
    }

    println!(
        "[kernel] ---------- hart {} start to fetch task... ---------- ",
        hart_id
    );
    loop {
        executor::run_until_idle();
        // #[cfg(feature = "multi_hart")]
        // {
        //     use crate::timer::current_time_duration;
        //     let start_ts = current_time_duration();
        //     loop {
        //         let current_ts = current_time_duration();
        //         if current_ts - start_ts > Duration::from_millis(2) {
        //             break;
        //         }
        //     }
        // }
    }
}

fn init_block_device() {
    {
        *BLOCK_DEVICE.lock() = Some(Arc::new(VirtIOBlock::new()));
    }
}

fn init_char_device() {
    {
        *CHAR_DEVICE.get_unchecked_mut() = Some(Box::new(qemu::uart::UART::new(
            0xffff_ffc0_1000_0000,
            Box::new(|ch| {
                TTY.get_unchecked_mut().as_ref().unwrap().handle_irq(ch);
            }),
        )));
    }
}

pub fn driver_init() {
    unsafe {
        KERNEL_PAGE_TABLE = Some(
            KERNEL_SPACE
                .as_ref()
                .expect("KERENL SPACE not init yet")
                .page_table
                .clone(),
        )
    };
    initplic(0xffff_ffc0_0c00_0000);
    init_char_device();
    init_block_device();
    interrupts::enable_external_interrupt();
}
