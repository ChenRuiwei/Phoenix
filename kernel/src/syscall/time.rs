use core::time::Duration;

use arch::time::{get_time_duration, get_time_ms, get_time_us};
use async_utils::{Select2Futures, SelectOutput};
use systype::{SysError, SysResult, SyscallResult};
use time::{
    timespec::TimeSpec, timeval::TimeVal, tms::TMS, CLOCK_DEVIATION, CLOCK_MONOTONIC,
    CLOCK_PROCESS_CPUTIME_ID, CLOCK_REALTIME, CLOCK_THREAD_CPUTIME_ID,
};
use timer::timelimited_task::ksleep_ms;

use crate::{
    mm::{UserReadPtr, UserWritePtr},
    processor::hart::current_task,
    task::signal::WaitHandlableSignal,
};

/// Retrieves the current time of day.
///
/// This function fetches the current time, writing it into a provided `TimeVal`
/// structure, pointed to by `tv`. The time is expressed in seconds and
/// microseconds since the Epoch (00:00:00 UTC on 1 January 1970). This function
/// does not provide timezone information, and the `tz` parameter is ignored.
///
/// # Parameters
/// - `tv`: `UserWritePtr<TimeVal>`
///   - A user-space pointer to a `TimeVal` structure where the syscall will
///     write the current time.
/// - `tz`: `usize`
///   - An obsolete parameter historically used for timezone information.
///     Typically set to zero or ignored in modern implementations.
pub fn sys_gettimeofday(tv: UserWritePtr<TimeVal>, _tz: usize) -> SyscallResult {
    if tv.not_null() {
        tv.write(current_task(), TimeVal::from_usec(get_time_us()))?;
    }
    Ok(0)
}

pub fn sys_times(tms: UserWritePtr<TMS>) -> SyscallResult {
    let task = current_task();
    if tms.not_null() {
        tms.write(task, TMS::from_task_time_stat(task.time_stat()))?;
    }
    Ok(0)
}

/// nanosleep suspends the execution of the calling thread until either at least
/// the time specified in *req has elapsed, or the delivery of a signal that
/// triggers the invocation of a handler in the calling thread or that
/// terminates the process
///
/// req: Specify the length of time you want to sleep
/// rem: If it is not NULL, when the function returns, the timespec structure it
/// points to will be updated to the time it has not finished sleeping
pub async fn sys_nanosleep(
    req: UserReadPtr<TimeSpec>,
    rem: UserWritePtr<TimeSpec>,
) -> SyscallResult {
    let task = current_task();
    if req.is_null() {
        log::info!("[sys_nanosleep] sleep request is null");
        return Ok(0);
    }
    let req = req.read(task)?;
    let sleep_ms = req.into_ms();
    let current_ms = get_time_ms();
    match Select2Futures::new(WaitHandlableSignal(task), ksleep_ms(sleep_ms)).await {
        SelectOutput::Output1(break_ms) => {
            log::info!("[sys_nanosleep] interrupt by signal");
            if !rem.is_null() {
                let remain_ms = sleep_ms - (break_ms - current_ms);
                rem.write(task, TimeSpec::from_ms(remain_ms))?;
            }
            Err(SysError::EINTR)
        }
        SelectOutput::Output2(_) => Ok(0),
    }
}

/// retrieve the time of the specified clock clockid
pub fn sys_clock_gettime(clockid: usize, tp: UserWritePtr<TimeSpec>) -> SyscallResult {
    if tp.is_null() {
        return Ok(0);
    }
    match clockid {
        CLOCK_REALTIME | CLOCK_MONOTONIC => {
            let current = get_time_duration();
            tp.write(
                current_task(),
                (unsafe { CLOCK_DEVIATION }[clockid] + current).into(),
            )?;
        }
        CLOCK_PROCESS_CPUTIME_ID => {
            let task = current_task();
            let mut cpu_time = task.time_stat().cpu_time();
            task.with_thread_group(|tg| {
                for thread in tg.iter() {
                    cpu_time += thread.time_stat().cpu_time();
                }
            });
            tp.write(task, cpu_time.into())?;
        }
        CLOCK_THREAD_CPUTIME_ID => {
            let task = current_task();
            tp.write(task, task.time_stat().cpu_time().into())?;
        }
        _ => {
            log::error!("[sys_clock_gettime] unsupported clockid{}", clockid);
            return Err(SysError::EINTR);
        }
    }
    Ok(0)
}

pub fn sys_clock_settime(clockid: usize, tp: UserReadPtr<TimeSpec>) -> SyscallResult {
    if clockid == CLOCK_PROCESS_CPUTIME_ID
        || clockid == CLOCK_THREAD_CPUTIME_ID
        || clockid == CLOCK_MONOTONIC
    {
        log::error!("[sys_clock_settime] The clockid {} specified in a call to clock_settime() is not a settable clock.", clockid);
        return Err(SysError::EINTR);
    }
    let task = current_task();
    let tp = tp.read(task)?;
    if !tp.is_valid() {
        return Err(SysError::EINTR);
    }
    match clockid {
        CLOCK_REALTIME => {
            if tp.into_ms() < get_time_ms() {
                log::error!("[sys_clock_settime] attempted to set the time to a value less than the current value of the CLOCK_MONOTONIC clock.");
                return Err(SysError::EINTR);
            }
            unsafe {
                CLOCK_DEVIATION[clockid] = Duration::from(tp) - get_time_duration();
            }
        }
        _ => {
            log::error!("[sys_clock_gettime] unsupported clockid{}", clockid);
            return Err(SysError::EINTR);
        }
    }
    Ok(0)
}

/// finds the resolution (precision) of the specified clock clockid
pub fn sys_clock_getres(_clockid: usize, res: UserWritePtr<TimeSpec>) -> SyscallResult {
    if res.is_null() {
        return Ok(0);
    }
    res.write(current_task(), Duration::from_nanos(1).into())?;
    Ok(0)
}
