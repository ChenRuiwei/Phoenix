use arch::time::{get_time_ms, get_time_us};
use async_utils::{Select2Futures, SelectOutput};
use systype::{SysError, SyscallResult};
use time::{timespec::TimeSpec, timeval::TimeVal, tms::TMS};
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
pub fn sys_gettimeofday(tv: UserWritePtr<TimeVal>, tz: usize) -> SyscallResult {
    tv.write(current_task(), TimeVal::from_usec(get_time_us()));
    Ok(0)
}

pub fn sys_times(tms: UserWritePtr<TMS>) -> SyscallResult {
    tms.write(
        current_task(),
        TMS::from_task_time_stat(current_task().time_stat()),
    );
    Ok(0)
}

/// nanosleep suspends  the execution of the calling thread until either at
/// least the time specified in *req has elapsed, or the delivery of a signal
/// that triggers the invocation of a handler in the calling thread or that
/// terminates the process
///
/// req: Specify the length of time you want to sleep
/// rem: If it is not NULL, when the function returns, the timespec structure it
/// points to will be updated to the time it has not finished sleeping
pub async fn sys_nanosleep(
    req: UserReadPtr<TimeSpec>,
    rem: UserWritePtr<TimeSpec>,
) -> SyscallResult {
    let req = req.read(current_task())?;
    let sleep_ms = req.into_ms();
    let current_ms = get_time_ms();
    match Select2Futures::new(WaitHandlableSignal(current_task()), ksleep_ms(sleep_ms)).await {
        SelectOutput::Output1(break_ms) => {
            log::info!("[sys_nanosleep] interrupt by signal");
            if !rem.is_null() {
                let remain_ms = sleep_ms - (break_ms - current_ms);
                rem.write(current_task(), TimeSpec::from_ms(remain_ms));
            }
            Err(SysError::EINTR)
        }
        SelectOutput::Output2(_) => Ok(0),
    }
}
