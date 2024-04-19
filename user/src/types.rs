pub use signal::sigset::Sig;
use signal::sigset::SigSet;
pub use time::timeval::TimeVal;
#[derive(Clone, Copy)]
#[repr(C)]
pub struct SigAction {
    /// sa_handler specifies the action to be associated with signum and can be
    /// one of the following:
    /// 1. SIG_DFL for the default action
    /// 2. SIG_IGN to ignore this signal
    /// 3. A pointer to a signal handling function. This function receives the
    ///    signal number as its only argument.
    pub sa_handler: usize,
    /// sa_mask specifies a mask of signals which should be blocked during
    /// execution of the signal handler.
    pub sa_mask: SigSet,
    pub sa_flags: usize,
}

bitflags! {
    pub struct OpenFlags: u32 {
        const O_RDONLY = 0;
        const O_WRONLY = 1 << 0;
        const O_RDWR = 1 << 1;
        const O_CLOEXEC = 1 << 7;
        const O_CREATE = 1 << 9;
        const O_TRUNC = 1 << 10;
    }
}
pub const AT_FDCWD: isize = -100;
