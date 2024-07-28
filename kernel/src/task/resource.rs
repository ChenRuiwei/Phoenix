use core::time::Duration;

use super::Task;

impl Task {
    pub fn get_process_ustime(&self) -> (Duration, Duration) {
        self.with_thread_group(|tg| -> (Duration, Duration) {
            tg.iter()
                .map(|thread| thread.time_stat().user_system_time())
                .reduce(|(acc_utime, acc_stime), (utime, stime)| {
                    (acc_utime + utime, acc_stime + stime)
                })
                .unwrap()
        })
    }

    pub fn get_process_utime(&self) -> Duration {
        self.with_thread_group(|tg| -> Duration {
            tg.iter()
                .map(|thread| thread.time_stat().user_time())
                .reduce(|acc_utime, utime| acc_utime + utime)
                .unwrap()
        })
    }

    pub fn get_process_cputime(&self) -> Duration {
        self.with_thread_group(|tg| -> Duration {
            tg.iter()
                .map(|thread| thread.time_stat().cpu_time())
                .reduce(|acc, cputime| acc + cputime)
                .unwrap()
        })
    }
}

bitflags! {
    #[derive(Clone, Copy)]
    #[repr(C)]
    pub struct CpuMask: usize {
        const CPU0 = 0b00000001;
        const CPU1 = 0b00000010;
        const CPU2 = 0b00000100;
        const CPU3 = 0b00001000;
        const CPU4 = 0b00010000;
        const CPU5 = 0b00100000;
        const CPU6 = 0b01000000;
        const CPU7 = 0b10000000;
        const CPU_ALL = 0b11111111;
    }
}
