#![no_std]
#![no_main]

extern crate user_lib;

use core::sync::atomic::{AtomicI32, Ordering};

use user_lib::*;

static FUTEX_ADDR: AtomicI32 = AtomicI32::new(0);
#[no_mangle]
fn main() {
    let futex_ptr = &FUTEX_ADDR as *const AtomicI32 as usize;

    let pid = create_thread(CloneFlags::THREAD);
    if pid == 0 {
        // Child process
        println!("Child: Waiting for signal...");
        futex(futex_ptr, FUTEX_WAIT, 0, 0, 0, 0);
        println!(
            "Child: Received signal! Value is now: {}",
            FUTEX_ADDR.load(Ordering::SeqCst)
        );
        exit(0);
    } else if pid > 0 {
        // Parent process
        println!("Parent: Forked child process with PID: {}", pid);
        sleep(1000); // Sleep for 1 second
        println!("Parent: Waking up child...");
        FUTEX_ADDR.store(1, Ordering::SeqCst);
        futex(futex_ptr, FUTEX_WAKE, 1, 0, 0, 0);
        println!(
            "Parent: Wake signal sent. Value is now: {}",
            FUTEX_ADDR.load(Ordering::SeqCst)
        );
    } else {
        println!("Fork failed");
    }
}
