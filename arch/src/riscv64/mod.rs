pub mod entry;
pub mod interrupts;
pub mod memory;
pub mod register;
pub mod sstatus;
pub mod time;

#[inline(never)]
pub fn spin(cycle: usize) {
    for _ in 0..cycle {
        core::hint::spin_loop();
    }
}
