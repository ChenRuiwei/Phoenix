use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;
use device_core::{CharDevice, DeviceMajor};
use driver::{get_device_manager, serial::Serial};
use spin::Once;
use strum::FromRepr;
use sync::mutex::{SleepLock, SpinNoIrqLock};
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{
    Dentry, DentryMeta, DirEntry, File, FileMeta, Inode, InodeMeta, InodeMode, PollEvents, Stat,
    SuperBlock,
};

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

    fn base_unlink(self: Arc<Self>, _name: &str) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }
}

pub struct TtyInode {
    meta: InodeMeta,
    char_dev: Arc<dyn CharDevice>,
}

impl TtyInode {
    pub fn new(super_block: Arc<dyn SuperBlock>) -> Arc<Self> {
        let mut meta = InodeMeta::new(InodeMode::CHAR, super_block, 0);
        let (&dev_id, char_dev) = get_device_manager()
            .devices()
            .iter()
            .filter(|(dev_id, _device)| dev_id.major == DeviceMajor::Serial)
            .next()
            .unwrap();
        meta.dev_id = Some(dev_id);
        let char_dev = char_dev
            .clone()
            .downcast_arc::<Serial>()
            .unwrap_or_else(|_| unreachable!());
        Arc::new(Self { meta, char_dev })
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
            st_mode: inner.mode.bits(),
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
    /// `sshd` might use?
    TIOCNOTTY = 0x5422,
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

pub static TTY: Once<Arc<TtyFile>> = Once::new();

pub struct TtyFile {
    meta: FileMeta,
    pub(crate) inner: SpinNoIrqLock<TtyInner>,
}

struct TtyInner {
    fg_pgid: Pid,
    win_size: WinSize,
    termios: Termios,
}

impl TtyFile {
    pub fn new(dentry: Arc<dyn Dentry>, inode: Arc<dyn Inode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry, inode),
            inner: SpinNoIrqLock::new(TtyInner {
                fg_pgid: 1 as u32,
                win_size: WinSize::new(),
                termios: Termios::new(),
            }),
        })
    }
}

#[async_trait]
impl File for TtyFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, _offset: usize, buf: &mut [u8]) -> SyscallResult {
        log::debug!("[TtyFile::base_read_at] buf len {}", buf.len());
        let char_dev = &self
            .inode()
            .downcast_arc::<TtyInode>()
            .unwrap_or_else(|_| unreachable!())
            .char_dev;
        let len = char_dev.read(buf).await;
        let termios = self.inner.lock().termios;
        if termios.is_icrnl() {
            for i in 0..len {
                if buf[i] == '\r' as u8 {
                    buf[i] = '\n' as u8;
                }
            }
        }
        if termios.is_echo() {
            self.base_write_at(0, buf).await;
        }
        Ok(len)
    }

    async fn base_write_at(&self, _offset: usize, buf: &[u8]) -> SyscallResult {
        let char_dev = &self
            .inode()
            .downcast_arc::<TtyInode>()
            .unwrap_or_else(|_| unreachable!())
            .char_dev;
        let len = char_dev.write(buf).await;
        Ok(len)
    }

    async fn base_poll(&self, events: PollEvents) -> PollEvents {
        let mut res = PollEvents::empty();
        let char_dev = &self
            .inode()
            .downcast_arc::<TtyInode>()
            .unwrap_or_else(|_| unreachable!())
            .char_dev;
        if events.contains(PollEvents::IN) {
            if char_dev.poll_in().await {
                res |= PollEvents::IN;
            }
        }
        if events.contains(PollEvents::OUT) {
            if char_dev.poll_out().await {
                res |= PollEvents::OUT;
            }
        }
        log::debug!("[TtyFile::base_poll] ret events:{res:?}");
        res
    }

    /// See `ioctl_tty` manual page.
    fn ioctl(&self, cmd: usize, arg: usize) -> SyscallResult {
        use TtyIoctlCmd::*;
        let Some(cmd) = TtyIoctlCmd::from_repr(cmd) else {
            log::error!("[TtyFile::ioctl] cmd {cmd} not included");
            unimplemented!()
        };
        log::debug!("[TtyFile::ioctl] cmd {:?}, value {:#x}", cmd, arg);
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
                    log::info!("termios {:#x?}", self.inner.lock().termios);
                }
                Ok(0)
            }
            TIOCGPGRP => {
                let fg_pgid = self.inner.lock().fg_pgid;
                log::debug!("[TtyFile::ioctl] get fg pgid {fg_pgid}");
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
                log::debug!("[TtyFile::ioctl] set fg pgid {fg_pgid}");
                Ok(0)
            }
            TIOCGWINSZ => {
                let win_size = self.inner.lock().win_size;
                log::debug!("[TtyFile::ioctl] get window size {win_size:?}",);
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
            TIOCNOTTY => Ok(0),
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
#[derive(Debug, Clone, Copy)]
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

    fn is_icrnl(&self) -> bool {
        const ICRNL: u32 = 0o0000400;
        self.iflag & ICRNL != 0
    }

    fn is_echo(&self) -> bool {
        const ECHO: u32 = 0o0000010;
        self.lflag & ECHO != 0
    }
}
