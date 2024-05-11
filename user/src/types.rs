pub use signal::sigset::Sig;
use signal::sigset::SigSet;
pub use time::{timespec::TimeSpec, timeval::TimeVal};
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct SigAction {
    /// sa_handler specifies the action to be associated with signum and can be
    /// one of the following:
    /// 1. SIG_DFL for the default action
    /// 2. SIG_IGN to ignore this signal
    /// 3. A pointer to a signal handling function. This function receives the
    ///    signal number as its only argument.
    pub sa_handler: usize,
    pub sa_flags: SigActionFlag,
    pub restorer: usize,
    /// sa_mask specifies a mask of signals which should be blocked during
    /// execution of the signal handler.
    pub sa_mask: SigSet,
}

bitflags! {
    #[derive(Default, Copy, Clone)]
    pub struct SigActionFlag : usize {
        const SA_NOCLDSTOP = 1;
        const SA_NOCLDWAIT = 2;
        const SA_SIGINFO = 4;
        const SA_ONSTACK = 0x08000000;
        const SA_RESTART = 0x10000000;
        const SA_NODEFER = 0x40000000;
        const SA_RESETHAND = 0x80000000;
        const SA_RESTORER = 0x04000000;
    }
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
