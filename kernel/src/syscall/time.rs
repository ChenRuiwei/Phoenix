use core::time::Duration;

use arch::time::{get_time_duration, get_time_ms, get_time_us};
use systype::{SysError, SyscallResult};
use time::{
    timespec::TimeSpec,
    timeval::{ITimerVal, TimeVal},
    tms::TMS,
    CLOCK_DEVIATION, CLOCK_MONOTONIC, CLOCK_PROCESS_CPUTIME_ID, CLOCK_REALTIME,
    CLOCK_THREAD_CPUTIME_ID,
};

use super::Syscall;
use crate::mm::{UserReadPtr, UserWritePtr};

impl Syscall<'_> {
    /// Retrieves the current time of day.
    ///
    /// This function fetches the current time, writing it into a provided
    /// `TimeVal` structure, pointed to by `tv`. The time is expressed in
    /// seconds and microseconds since the Epoch (00:00:00 UTC on 1 January
    /// 1970). This function does not provide timezone information, and the
    /// `tz` parameter is ignored.
    ///
    /// # Parameters
    /// - `tv`: `UserWritePtr<TimeVal>`
    ///   - A user-space pointer to a `TimeVal` structure where the syscall will
    ///     write the current time.
    /// - `tz`: `usize`
    ///   - An obsolete parameter historically used for timezone information.
    ///     Typically set to zero or ignored in modern implementations.
    pub fn sys_gettimeofday(&self, tv: UserWritePtr<TimeVal>, _tz: usize) -> SyscallResult {
        let task = self.task;
        if tv.not_null() {
            tv.write(&task, TimeVal::from_usec(get_time_us()))?;
        }
        Ok(0)
    }

    pub fn sys_times(&self, tms: UserWritePtr<TMS>) -> SyscallResult {
        let task = self.task;
        if tms.not_null() {
            tms.write(&task, TMS::from_task_time_stat(task.time_stat()))?;
        }
        Ok(0)
    }

    /// nanosleep suspends the execution of the calling thread until either at
    /// least the time specified in *req has elapsed, or the delivery of a
    /// signal that triggers the invocation of a handler in the calling
    /// thread or that terminates the process
    ///
    /// req: Specify the length of time you want to sleep
    /// rem: If it is not NULL, when the function returns, the timespec
    /// structure it points to will be updated to the time it has not
    /// finished sleeping
    pub async fn sys_nanosleep(
        &self,
        req: UserReadPtr<TimeSpec>,
        rem: UserWritePtr<TimeSpec>,
    ) -> SyscallResult {
        let task = self.task;
        if req.is_null() {
            log::info!("[sys_nanosleep] sleep request is null");
            return Ok(0);
        }
        let req = req.read(&task)?;
        let remain = task.suspend_timeout(req.into()).await;
        if remain.is_zero() {
            Ok(0)
        } else {
            if rem.not_null() {
                rem.write(&task, remain.into())?;
            }
            Err(SysError::EINTR)
        }
    }

    /// retrieve the time of the specified clock clockid
    pub fn sys_clock_gettime(&self, clockid: usize, tp: UserWritePtr<TimeSpec>) -> SyscallResult {
        if tp.is_null() {
            return Ok(0);
        }
        let task = self.task;
        match clockid {
            CLOCK_REALTIME | CLOCK_MONOTONIC => {
                let current = get_time_duration();
                tp.write(
                    &task,
                    (unsafe { CLOCK_DEVIATION }[clockid] + current).into(),
                )?;
            }
            CLOCK_PROCESS_CPUTIME_ID => {
                let cpu_time = task.get_process_cputime();
                tp.write(&task, cpu_time.into())?;
            }
            CLOCK_THREAD_CPUTIME_ID => {
                tp.write(&task, task.time_stat().cpu_time().into())?;
            }
            _ => {
                log::error!("[sys_clock_gettime] unsupported clockid{}", clockid);
                return Err(SysError::EINTR);
            }
        }
        Ok(0)
    }

    pub fn sys_clock_settime(&self, clockid: usize, tp: UserReadPtr<TimeSpec>) -> SyscallResult {
        if clockid == CLOCK_PROCESS_CPUTIME_ID
            || clockid == CLOCK_THREAD_CPUTIME_ID
            || clockid == CLOCK_MONOTONIC
        {
            log::error!("[sys_clock_settime] The clockid {} specified in a call to clock_settime() is not a settable clock.", clockid);
            return Err(SysError::EINVAL);
        }
        let task = self.task;
        let tp = tp.read(&task)?;
        if !tp.is_valid() {
            return Err(SysError::EINVAL);
        }
        match clockid {
            CLOCK_REALTIME => {
                if tp.into_ms() < get_time_ms() {
                    log::error!("[sys_clock_settime] attempted to set the time to a value less than the current value of the CLOCK_MONOTONIC clock.");
                    return Err(SysError::EINVAL);
                }
                unsafe {
                    CLOCK_DEVIATION[clockid] = Duration::from(tp) - get_time_duration();
                }
            }
            _ => {
                log::error!("[sys_clock_gettime] unsupported clockid{}", clockid);
                return Err(SysError::EINVAL);
            }
        }
        Ok(0)
    }

    /// finds the resolution (precision) of the specified clock clockid
    pub fn sys_clock_getres(&self, _clockid: usize, res: UserWritePtr<TimeSpec>) -> SyscallResult {
        if res.is_null() {
            return Ok(0);
        }
        let task = self.task;
        res.write(&task, Duration::from_nanos(1).into())?;
        Ok(0)
    }

    pub async fn sys_clock_nanosleep(
        &self,
        clockid: usize,
        flags: usize,
        t: UserReadPtr<TimeSpec>,
        rem: UserWritePtr<TimeSpec>,
    ) -> SyscallResult {
        /// for clock_nanosleep
        pub const TIMER_ABSTIME: usize = 1;
        let task = self.task;
        match clockid {
            // FIXME: what is CLOCK_MONOTONIC
            CLOCK_REALTIME | CLOCK_MONOTONIC => {
                let ts = t.read(task)?;
                let req: Duration = ts.into();
                let remain = if flags == TIMER_ABSTIME {
                    let current = get_time_duration();
                    // request time is absolutely
                    if req.le(&current) {
                        return Ok(0);
                    }
                    let sleep = req - current;
                    task.suspend_timeout(req).await
                } else {
                    task.suspend_timeout(req).await
                };
                if remain.is_zero() {
                    Ok(0)
                } else {
                    if rem.not_null() {
                        rem.write(&task, remain.into())?;
                    }
                    Err(SysError::EINTR)
                }
            }
            _ => {
                log::error!("[sys_clock_nanosleep] unsupported clockid {}", clockid);
                return Err(SysError::EINVAL);
            }
        }
    }

    /// provide access to interval timers, that is, timers that initially expire
    /// at some point in the future, and (optionally) at regular intervals
    /// after that. When a timer expires, a signal is generated for the
    /// calling process, and the timer is reset to the specified interval
    /// (if the interval is nonzero). Three  types  of  timers—specified via
    /// the which argument—are provided, each of which counts against a
    /// different clock and generates a different signal
    /// on timer expiration:
    pub fn sys_setitimer(
        &self,
        which: i32,
        new_value: UserReadPtr<ITimerVal>,
        old_value: UserWritePtr<ITimerVal>,
    ) -> SyscallResult {
        if which < 0 || which > 2 {
            return Err(SysError::EINVAL);
        }
        let task = self.task;
        let new = new_value.read(&task)?;
        if !new.is_valid() {
            return Err(SysError::EINVAL);
        }
        let old = task.with_mut_itimers(|itimers| itimers[which as usize].set(new));
        if old_value.not_null() {
            old_value.write(&task, old)?;
        }
        Ok(0)
    }

    pub fn sys_getitimer(&self, which: i32, curr_value: UserWritePtr<ITimerVal>) -> SyscallResult {
        if which < 0 || which > 2 {
            return Err(SysError::EINVAL);
        }
        if curr_value.not_null() {
            let task = self.task;
            let itimerval = task.with_itimers(|itimers| itimers[which as usize].get());
            curr_value.write(&task, itimerval)?;
        }
        Ok(0)
    }
}
