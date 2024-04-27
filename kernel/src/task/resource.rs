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
