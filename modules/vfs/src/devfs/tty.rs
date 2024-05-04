use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::{
    future::Future,
    mem::MaybeUninit,
    pin::Pin,
    task::{Poll, Waker},
};

use async_trait::async_trait;
use async_utils::{take_waker, yield_now};
use config::process::INIT_PROC_PID;
use driver::{getchar, print, CHAR_DEVICE};
use spin::Once;
use sync::mutex::{SleepLock, SpinNoIrqLock};
use systype::{SysResult, SyscallResult};
use vfs_core::{Dentry, File, FileMeta, Inode, InodeMeta, InodeMode, Path};

use crate::sys_root_dentry;

pub struct TtyInode {
    meta: InodeMeta,
}

impl TtyInode {
    pub fn new(parent: Arc<dyn Inode>, path: &str) -> Self {
        let meta = InodeMeta::new(InodeMode::CHAR, Arc::<usize>::new_zeroed(), 0);
        Self { meta }
    }
}

impl Inode for TtyInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> systype::SysResult<vfs_core::Stat> {
        todo!()
    }
}

const PRINT_LOCKED: bool = false;

static PRINT_MUTEX: SleepLock<bool> = SleepLock::new(false);

type Pid = u32;

// For struct termios
/// Gets the current serial port settings.
const TCGETS: usize = 0x5401;
/// Sets the serial port settings immediately.
const TCSETS: usize = 0x5402;
/// Sets the serial port settings after allowing the input and output buffers to
/// drain/empty.
const TCSETSW: usize = 0x5403;
/// Sets the serial port settings after flushing the input and output buffers.
const TCSETSF: usize = 0x5404;
/// For struct termio
/// Gets the current serial port settings.
const TCGETA: usize = 0x5405;
/// Sets the serial port settings immediately.
#[allow(unused)]
const TCSETA: usize = 0x5406;
/// Sets the serial port settings after allowing the input and output buffers to
/// drain/empty.
#[allow(unused)]
const TCSETAW: usize = 0x5407;
/// Sets the serial port settings after flushing the input and output buffers.
#[allow(unused)]
const TCSETAF: usize = 0x5408;
/// If the terminal is using asynchronous serial data transmission, and arg is
/// zero, then send a break (a stream of zero bits) for between 0.25 and 0.5
/// seconds.
const TCSBRK: usize = 0x5409;
/// Get the process group ID of the foreground process group on this terminal.
const TIOCGPGRP: usize = 0x540F;
/// Set the foreground process group ID of this terminal.
const TIOCSPGRP: usize = 0x5410;
/// Get window size.
const TIOCGWINSZ: usize = 0x5413;
/// Set window size.
const TIOCSWINSZ: usize = 0x5414;
/// Non-cloexec
#[allow(unused)]
const FIONCLEX: usize = 0x5450;
/// Cloexec
#[allow(unused)]
const FIOCLEX: usize = 0x5451;
/// rustc using pipe and ioctl pipe file with this request id
/// for non-blocking/blocking IO control setting
#[allow(unused)]
const FIONBIO: usize = 0x5421;
/// Read time
#[allow(unused)]
const RTC_RD_TIME: usize = 0x80247009;

#[repr(C)]
#[derive(Clone, Copy)]
struct WinSize {
    ws_row: u16,
    ws_col: u16,
    xpixel: u16,
    ypixel: u16,
}

impl WinSize {
    fn new() -> Self {
        Self {
            // ws_row: 67,
            // ws_col: 270,
            ws_row: 67,
            ws_col: 120,
            xpixel: 0,
            ypixel: 0,
        }
    }
}

pub fn init() -> SysResult<()> {
    let path = "/dev/tty";
    let path = Path::new(sys_root_dentry(), sys_root_dentry(), path);
    let tty_dentry = path.walk(InodeMode::empty())?;
    let parent = tty_dentry.parent().unwrap();
    parent.create("tty", InodeMode::CHAR)?;
    // let tty_file = tty_dentry.open()?;
    let tty_file = TtyFile::new(tty_dentry.clone(), tty_dentry.inode()?);
    TTY.call_once(|| Arc::new(tty_file));
    Ok(())
}

const CTRL_C: u8 = 3;

pub static TTY: Once<Arc<TtyFile>> = Once::new();

const QUEUE_BUFFER_LEN: usize = 256;

struct QueueBuffer {
    buf: [u8; QUEUE_BUFFER_LEN],
    e: usize,
    f: usize,
}

impl QueueBuffer {
    fn new() -> Self {
        Self {
            buf: [0; QUEUE_BUFFER_LEN],
            e: 0,
            f: 0,
        }
    }
    fn push(&mut self, val: u8) {
        self.buf[self.f] = val;
        self.f = (self.f + 1) % QUEUE_BUFFER_LEN;
    }
    fn top(&self) -> u8 {
        if self.e == self.f {
            0xff
        } else {
            self.buf[self.e]
        }
    }

    fn pop(&mut self) -> u8 {
        if self.e == self.f {
            0xff
        } else {
            let ret = self.buf[self.e];
            self.e = (self.e + 1) % QUEUE_BUFFER_LEN;
            ret
        }
    }
}

pub struct TtyFile {
    /// Temporarily save poll in data
    buf: SpinNoIrqLock<QueueBuffer>,
    meta: FileMeta,
    inner: SpinNoIrqLock<TtyInner>,
}

struct TtyInner {
    fg_pgid: Pid,
    win_size: WinSize,
    termios: Termios,
}

impl TtyFile {
    pub fn new(dentry: Arc<dyn Dentry>, inode: Arc<dyn Inode>) -> Self {
        Self {
            buf: SpinNoIrqLock::new(QueueBuffer::new()),
            meta: FileMeta::new(dentry, inode),
            inner: SpinNoIrqLock::new(TtyInner {
                fg_pgid: INIT_PROC_PID as u32,
                win_size: WinSize::new(),
                termios: Termios::new(),
            }),
        }
    }

    pub fn handle_irq(&self, ch: u8) {
        todo!()
        // log::debug!("[TtyFile::handle_irq] handle irq, ch {}", ch);
        // self.buf.lock().push(ch);
        // if ch == CTRL_C {
        //     let pids =
        // PROCESS_GROUP_MANAGER.get_group_by_pgid(self.inner.lock().fg_pgid as
        // usize);     log::debug!("[TtyFile::handle_irq] fg pid {}",
        // self.inner.lock().fg_pgid);     for pid in pids {
        //         let process = PROCESS_MANAGER.get(pid);
        //         if let Some(p) = process {
        //             p.inner_handler(|proc| {
        //                 for (_, thread) in proc.threads.iter() {
        //                     if let Some(t) = thread.upgrade() {
        //                         log::debug!("[TtyFile::handle_irq] kill tid
        // {}", t.tid());                         t.recv_signal(SIGINT);
        //                     }
        //                 }
        //             })
        //         }
        //     }
        // }
    }
}

#[async_trait]
impl File for TtyFile {
    async fn read(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        let mut cnt = 0;
        loop {
            let ch: u8;
            let self_buf = self.buf.lock().pop();
            if self_buf != 0xff {
                ch = self_buf;
            } else {
                ch = getchar();
                if ch == 0xff {
                    CHAR_DEVICE
                        .get()
                        .unwrap()
                        .register_waker(take_waker().await);
                    log::debug!("[TtyFuture::poll] nothing to read");
                    yield_now().await;
                }
            }
            log::debug!(
                "[TtyFuture::poll] recv ch {}, cnt {}, len {}",
                ch,
                cnt,
                buf.len()
            );
            buf[cnt] = ch;

            cnt += 1;

            if cnt < buf.len() {
                yield_now().await;
            } else {
                return Ok(buf.len());
            }
        }
    }

    async fn write(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        let utf8_buf: Vec<u8> = buf.iter().filter(|c| c.is_ascii()).map(|c| *c).collect();
        if PRINT_LOCKED {
            let _locked = PRINT_MUTEX.lock().await;
            print!("{}", unsafe { core::str::from_utf8_unchecked(&utf8_buf) });
        } else {
            print!("{}", unsafe { core::str::from_utf8_unchecked(&utf8_buf) });
        }
        Ok(buf.len())
    }

    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn ioctl(&self, cmd: usize, arg: usize) -> SyscallResult {
        log::info!("[TtyFile::ioctl] command {:#x}, value {:#x}", cmd, arg);
        match cmd {
            TCGETS | TCGETA => {
                unsafe {
                    *(arg as *mut Termios) = self.inner.lock().termios;
                }
                Ok(0)
            }
            TCSETS | TCSETSW | TCSETSF => {
                unsafe {
                    self.inner.lock().termios = *(arg as *const Termios);
                }
                Ok(0)
            }
            TIOCGPGRP => {
                unsafe {
                    *(arg as *mut Pid) = self.inner.lock().fg_pgid;
                    log::info!("[TtyFile::ioctl] get fg pgid {}", *(arg as *const Pid));
                }
                Ok(0)
            }
            TIOCSPGRP => {
                unsafe {
                    log::info!("[TtyFile::ioctl] set fg pgid {}", *(arg as *const Pid));
                    self.inner.lock().fg_pgid = *(arg as *const Pid);
                }
                Ok(0)
            }
            TIOCGWINSZ => {
                unsafe {
                    *(arg as *mut WinSize) = self.inner.lock().win_size;
                }
                Ok(0)
            }
            TIOCSWINSZ => {
                unsafe {
                    self.inner.lock().win_size = *(arg as *const WinSize);
                }
                Ok(0)
            }
            TCSBRK => Ok(0),
            _ => todo!(),
        }
    }

    fn read_dir(&self) -> SysResult<Option<vfs_core::DirEntry>> {
        todo!()
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Termios {
    /// Input modes
    pub iflag: u32,
    /// Ouput modes
    pub oflag: u32,
    /// Control modes
    pub cflag: u32,
    /// Local modes
    pub lflag: u32,
    pub line: u8,
    /// Terminal special characters.
    pub cc: [u8; 19],
    // pub cc: [u8; 32],
    // pub ispeed: u32,
    // pub ospeed: u32,
}

impl Termios {
    fn new() -> Self {
        Self {
            // IMAXBEL | IUTF8 | IXON | IXANY | ICRNL | BRKINT
            iflag: 0o66402,
            // OPOST | ONLCR
            oflag: 0o5,
            // HUPCL | CREAD | CSIZE | EXTB
            cflag: 0o2277,
            // IEXTEN | ECHOTCL | ECHOKE ECHO | ECHOE | ECHOK | ISIG | ICANON
            lflag: 0o105073,
            line: 0,
            cc: [
                3,   // VINTR Ctrl-C
                28,  // VQUIT
                127, // VERASE
                21,  // VKILL
                4,   // VEOF Ctrl-D
                0,   // VTIME
                1,   // VMIN
                0,   // VSWTC
                17,  // VSTART
                19,  // VSTOP
                26,  // VSUSP Ctrl-Z
                255, // VEOL
                18,  // VREPAINT
                15,  // VDISCARD
                23,  // VWERASE
                22,  // VLNEXT
                255, // VEOL2
                0, 0,
            ],
            // ispeed: 0,
            // ospeed: 0,
        }
    }
}
