use core::fmt;

use bitflags::*;

pub const NSIG: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Sig(i32);

/// Sig为0时表示空信号，从1开始才是有含义的信号
impl Sig {
    pub const SIGHUP: Sig = Sig(1); // Hangup detected on controlling terminal or death of controlling process
    pub const SIGINT: Sig = Sig(2); // Interrupt from keyboard
    pub const SIGQUIT: Sig = Sig(3); // Quit from keyboard
    pub const SIGILL: Sig = Sig(4); // Illegal Instruction
    pub const SIGTRAP: Sig = Sig(5); // Trace/breakpoint trap
    pub const SIGABRT: Sig = Sig(6); // Abort signal from abort(3)
    pub const SIGBUS: Sig = Sig(7); // Bus error (bad memory access)
    pub const SIGFPE: Sig = Sig(8); // Floating point exception
    pub const SIGKILL: Sig = Sig(9); // Kill signal
    pub const SIGUSR1: Sig = Sig(10); // User-defined signal 1
    pub const SIGSEGV: Sig = Sig(11); // Invalid memory reference
    pub const SIGUSR2: Sig = Sig(12); // User-defined signal 2
    pub const SIGPIPE: Sig = Sig(13); // Broken pipe: write to pipe with no readers
    pub const SIGALRM: Sig = Sig(14); // Timer signal from alarm(2)
    pub const SIGTERM: Sig = Sig(15); // Termination signal
    pub const SIGSTKFLT: Sig = Sig(16); // Stack fault on coprocessor (unused)
    pub const SIGCHLD: Sig = Sig(17); // Child stopped or terminated
    pub const SIGCONT: Sig = Sig(18); // Continue if stopped
    pub const SIGSTOP: Sig = Sig(19); // Stop process
    pub const SIGTSTP: Sig = Sig(20); // Stop typed at terminal
    pub const SIGTTIN: Sig = Sig(21); // Terminal input for background process
    pub const SIGTTOU: Sig = Sig(22); // Terminal output for background process
    pub const SIGURG: Sig = Sig(23); // Urgent condition on socket (4.2BSD)
    pub const SIGXCPU: Sig = Sig(24); // CPU time limit exceeded (4.2BSD)
    pub const SIGXFSZ: Sig = Sig(25); // File size limit exceeded (4.2BSD)
    pub const SIGVTALRM: Sig = Sig(26); // Virtual alarm clock (4.2BSD)
    pub const SIGPROF: Sig = Sig(27); // Profiling alarm clock
    pub const SIGWINCH: Sig = Sig(28); // Window resize signal (4.3BSD, Sun)
    pub const SIGIO: Sig = Sig(29); // I/O now possible (4.2BSD)
    pub const SIGPWR: Sig = Sig(30); // Power failure (System V)
    pub const SIGSYS: Sig = Sig(31); // Bad system call (SVr4); unused on Linux
    pub const SIGLEGACYMAX: Sig = Sig(32); // Legacy maximum signal
    pub const SIGMAX: Sig = Sig(64); // Maximum signal

    pub fn from_i32(signum: i32) -> Sig {
        Sig(signum as i32)
    }

    pub fn is_valid(&self) -> bool {
        self.0 >= 0 && self.0 < NSIG as i32
    }

    pub fn raw(&self) -> usize {
        self.0 as usize
    }

    pub fn index(&self) -> usize {
        (self.0 - 1) as usize
    }

    pub fn is_kill_or_stop(&self) -> bool {
        matches!(*self, Sig::SIGKILL | Sig::SIGSTOP)
    }
}

impl fmt::Display for Sig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for Sig {
    fn from(item: usize) -> Self {
        Sig(item as i32) // 这里假设usize到i32的转换是安全的，但要注意溢出的风险
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, Default)]
    pub struct SigSet: u64 {
        const SIGHUP    = 1 << 0 ;
        const SIGINT    = 1 << 1 ;
        const SIGQUIT   = 1 << 2 ;
        const SIGILL    = 1 << 3 ;
        const SIGTRAP   = 1 << 4 ;
        const SIGABRT   = 1 << 5 ;
        const SIGBUS    = 1 << 6 ;
        const SIGFPE    = 1 << 7 ;
        const SIGKILL   = 1 << 8 ;
        const SIGUSR1   = 1 << 9 ;
        const SIGSEGV   = 1 << 10;
        const SIGUSR2   = 1 << 11;
        const SIGPIPE   = 1 << 12;
        const SIGALRM   = 1 << 13;
        const SIGTERM   = 1 << 14;
        const SIGSTKFLT = 1 << 15;
        const SIGCHLD   = 1 << 16;
        const SIGCONT   = 1 << 17;
        const SIGSTOP   = 1 << 18;
        const SIGTSTP   = 1 << 19;
        const SIGTTIN   = 1 << 20;
        const SIGTTOU   = 1 << 21;
        const SIGURG    = 1 << 22;
        const SIGXCPU   = 1 << 23;
        const SIGXFSZ   = 1 << 24;
        const SIGVTALRM = 1 << 25;
        const SIGPROF   = 1 << 26;
        const SIGWINCH  = 1 << 27;
        const SIGIO     = 1 << 28;
        const SIGPWR    = 1 << 29;
        const SIGSYS    = 1 << 30;
        const SIGLEGACYMAX  = 1 << 31;
        const SIGMAX   = 1 << 63;
    }
}

impl SigSet {
    pub fn add_signal(&mut self, sig: Sig) {
        self.insert(SigSet::from_bits(1 << sig.index()).unwrap())
    }

    pub fn contain_signal(&self, sig: Sig) -> bool {
        self.contains(SigSet::from_bits(1 << sig.index()).unwrap())
    }

    pub fn remove_signal(&mut self, sig: Sig) {
        self.remove(SigSet::from_bits(1 << sig.index()).unwrap())
    }
}
