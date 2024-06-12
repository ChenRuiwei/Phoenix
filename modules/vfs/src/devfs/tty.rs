use alloc::{boxed::Box, sync::Arc, vec::Vec};

use async_trait::async_trait;
use async_utils::{take_waker, yield_now};
use driver::{getchar, print, CHAR_DEVICE};
use spin::Once;
use strum::FromRepr;
use sync::mutex::{SleepLock, SpinNoIrqLock};
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{
    Dentry, DentryMeta, DirEntry, File, FileMeta, Inode, InodeMeta, InodeMode, Path, Stat,
    SuperBlock,
};

use crate::sys_root_dentry;

pub struct TtyDentry {
    meta: DentryMeta,
}

impl TtyDentry {
    pub fn new(
        name: &str,
        super_block: Arc<dyn SuperBlock>,
        parent: Option<Arc<dyn Dentry>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new(name, super_block, parent),
        })
    }
}

impl Dentry for TtyDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(TtyFile::new(self.clone(), self.inode()?))
    }

    fn base_lookup(self: Arc<Self>, _name: &str) -> SysResult<Arc<dyn Dentry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_create(self: Arc<Self>, _name: &str, _mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_remove(self: Arc<Self>, _name: &str) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }
}

pub struct TtyInode {
    meta: InodeMeta,
}

impl TtyInode {
    pub fn new(super_block: Arc<dyn SuperBlock>) -> Arc<Self> {
        let meta = InodeMeta::new(InodeMode::CHAR, super_block, 0);
        Arc::new(Self { meta })
    }
}

impl Inode for TtyInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: self.meta.mode.bits(),
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: inner.size as u64,
            st_blksize: 0,
            __pad2: 0,
            st_blocks: 0 as u64,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }
}

const PRINT_LOCKED: bool = false;

static PRINT_MUTEX: SleepLock<bool> = SleepLock::new(false);

type Pid = u32;

/// Defined in <asm-generic/ioctls.h>
#[derive(FromRepr, Debug)]
#[repr(usize)]
enum TtyIoctlCmd {
    // For struct termios
    /// Gets the current serial port settings.
    TCGETS = 0x5401,
    /// Sets the serial port settings immediately.
    TCSETS = 0x5402,
    /// Sets the serial port settings after allowing the input and output
    /// buffers to drain/empty.
    TCSETSW = 0x5403,
    /// Sets the serial port settings after flushing the input and output
    /// buffers.
    TCSETSF = 0x5404,
    /// For struct termio
    /// Gets the current serial port settings.
    TCGETA = 0x5405,
    /// Sets the serial port settings immediately.
    #[allow(unused)]
    TCSETA = 0x5406,
    /// Sets the serial port settings after allowing the input and output
    /// buffers to drain/empty.
    #[allow(unused)]
    TCSETAW = 0x5407,
    /// Sets the serial port settings after flushing the input and output
    /// buffers.
    #[allow(unused)]
    TCSETAF = 0x5408,
    /// If the terminal is using asynchronous serial data transmission, and arg
    /// is zero, then send a break (a stream of zero bits) for between 0.25
    /// and 0.5 seconds.
    TCSBRK = 0x5409,
    /// Get the process group ID of the foreground process group on this
    /// terminal.
    TIOCGPGRP = 0x540F,
    /// Set the foreground process group ID of this terminal.
    TIOCSPGRP = 0x5410,
    /// Get window size.
    TIOCGWINSZ = 0x5413,
    /// Set window size.
    TIOCSWINSZ = 0x5414,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct WinSize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16, // Unused
    ws_ypixel: u16, // Unused
}

impl WinSize {
    fn new() -> Self {
        Self {
            ws_row: 67,
            ws_col: 120,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }
    }
}

pub fn init() -> SysResult<()> {
    let path = "/dev/tty";
    let path = Path::new(sys_root_dentry(), sys_root_dentry(), path);
    let tty_dentry = path.walk()?;
    let parent = tty_dentry.parent().unwrap();
    let sb = parent.super_block();
    let tty_dentry = TtyDentry::new("tty", sb.clone(), Some(parent.clone()));
    parent.insert(tty_dentry.clone());
    let tty_inode = TtyInode::new(sb.clone());
    tty_dentry.set_inode(tty_inode);
    let tty_file = TtyFile::new(tty_dentry.clone(), tty_dentry.inode()?);
    TTY.call_once(|| tty_file);
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
    pub fn new(dentry: Arc<dyn Dentry>, inode: Arc<dyn Inode>) -> Arc<Self> {
        Arc::new(Self {
            buf: SpinNoIrqLock::new(QueueBuffer::new()),
            meta: FileMeta::new(dentry, inode),
            inner: SpinNoIrqLock::new(TtyInner {
                fg_pgid: 2 as u32,
                win_size: WinSize::new(),
                termios: Termios::new(),
            }),
        })
    }

    pub fn handle_irq(&self, _ch: u8) {
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
    async fn base_read_at(&self, _offset: usize, buf: &mut [u8]) -> SyscallResult {
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
                    continue;
                }
            }
            log::debug!(
                "[TtyFuture::poll] recv ch {ch}, cnt {cnt}, len {}",
                buf.len()
            );
            buf[cnt] = ch;

            cnt += 1;

            if cnt < buf.len() {
                yield_now().await;
                continue;
            } else {
                return Ok(buf.len());
            }
        }
    }

    async fn base_write_at(&self, _offset: usize, buf: &[u8]) -> SyscallResult {
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

    /// See `ioctl_tty` manual page.
    fn ioctl(&self, cmd: usize, arg: usize) -> SyscallResult {
        use TtyIoctlCmd::*;
        let Some(cmd) = TtyIoctlCmd::from_repr(cmd) else {
            log::error!("[TtyFile::ioctl] cmd {cmd} not included");
            unimplemented!()
        };
        log::info!("[TtyFile::ioctl] cmd {:?}, value {:#x}", cmd, arg);
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
                let fg_pgid = self.inner.lock().fg_pgid;
                log::info!("[TtyFile::ioctl] get fg pgid {fg_pgid}");
                unsafe {
                    *(arg as *mut Pid) = fg_pgid;
                }
                Ok(0)
            }
            TIOCSPGRP => {
                unsafe {
                    self.inner.lock().fg_pgid = *(arg as *const Pid);
                }
                let fg_pgid = self.inner.lock().fg_pgid;
                log::info!("[TtyFile::ioctl] set fg pgid {fg_pgid}");
                Ok(0)
            }
            TIOCGWINSZ => {
                let win_size = self.inner.lock().win_size;
                log::info!("[TtyFile::ioctl] get window size {win_size:?}",);
                unsafe {
                    *(arg as *mut WinSize) = win_size;
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

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }
}

/// Defined in <asm-generic/termbits.h>
#[derive(Clone, Copy)]
#[repr(C)]
struct Termios {
    /// Input mode flags.
    pub iflag: u32,
    /// Output mode flags.
    pub oflag: u32,
    /// Control mode flags.
    pub cflag: u32,
    /// Local mode flags.
    pub lflag: u32,
    /// Line discipline.
    pub line: u8,
    /// control characters.
    pub cc: [u8; 19],
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
        }
    }
}
