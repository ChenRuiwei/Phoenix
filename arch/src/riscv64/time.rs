use config::{board::CLOCK_FREQ, time::INTERRUPTS_PER_SEC};
use riscv::register::time;

pub fn get_time() -> usize {
    time::read()
}

pub fn get_time_ms() -> usize {
    (time::read() * 1_000) / CLOCK_FREQ
}

pub fn get_time_sec() -> usize {
    time::read() / CLOCK_FREQ
}

pub fn get_time_us() -> usize {
    time::read() * 1_000_000 / CLOCK_FREQ
}

pub fn set_next_timer_irq() {
    let next_trigger: u64 = (get_time() + CLOCK_FREQ / INTERRUPTS_PER_SEC)
        .try_into()
        .unwrap();
    sbi_rt::set_timer(next_trigger);
}
