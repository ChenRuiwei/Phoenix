use arch::time::get_time_us;
use systype::SyscallResult;
use time::{timeval::TimeVal, tms::TMS};

use crate::{mm::UserWritePtr, processor::hart::current_task};

/// Retrieves the current time of day.
///
/// This function fetches the current time, writing it into a provided `TimeVal` structure,
/// pointed to by `tv`. The time is expressed in seconds and microseconds since the Epoch
/// (00:00:00 UTC on 1 January 1970). This function does not provide timezone information,
/// and the `tz` parameter is ignored.
///
/// # Parameters
/// - `tv`: `UserWritePtr<TimeVal>`
///   - A user-space pointer to a `TimeVal` structure where the syscall will write the current time.
/// - `tz`: `usize`
///   - An obsolete parameter historically used for timezone information. Typically set to zero
///     or ignored in modern implementations.
pub fn sys_gettimeofday(tv: UserWritePtr::<TimeVal>, tz: usize) -> SyscallResult {
    tv.write(current_task(), TimeVal::from_usec(get_time_us()));
    Ok(0)
}

pub fn sys_times(tms: UserWritePtr::<TMS>) -> SyscallResult {
    tms.write(current_task(), TMS::from_task_time_stat(current_task().get_time_stat()));
    Ok(0)
}