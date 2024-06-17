use alloc::{boxed::Box, collections::VecDeque, sync::Arc};
use core::{
    cmp,
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use async_trait::async_trait;
use async_utils::{get_waker, suspend_now, yield_now};
use config::fs::PIPE_BUF_CAPACITY;
use ringbuffer::{AllocRingBuffer, RingBuffer};
use sync::mutex::SpinNoIrqLock;
use systype::{SysError, SysResult};
use vfs_core::{
    arc_zero, File, FileMeta, FileSystemType, Inode, InodeMeta, InodeMode, PollEvents, Stat,
};

type Mutex<T> = SpinNoIrqLock<T>;

pub struct PipeInode {
    meta: InodeMeta,
    inner: Mutex<PipeInodeInner>,
}

// FIXME: multi thread read and write will be broken, e.g. is_closed
pub struct PipeInodeInner {
    is_write_closed: bool,
    is_read_closed: bool,
    buf: AllocRingBuffer<u8>,
    // WARN: `Waker` may not wake the task exactly, it may be abandoned. Rust only guarentees that
    // waker will wake the task from the last poll where the waker is passed in.
    read_waker: VecDeque<Waker>,
    write_waker: VecDeque<Waker>,
}

impl PipeInode {
    pub fn new() -> Arc<Self> {
        let meta = InodeMeta::new(
            InodeMode::FIFO,
            Arc::<usize>::new_uninit(),
            PIPE_BUF_CAPACITY,
        );
        let inner = Mutex::new(PipeInodeInner {
            is_write_closed: false,
            is_read_closed: false,
            buf: AllocRingBuffer::new(PIPE_BUF_CAPACITY),
            read_waker: VecDeque::new(),
            write_waker: VecDeque::new(),
        });
        Arc::new(Self { meta, inner })
    }
}

impl Inode for PipeInode {
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

struct PipeWritePollFuture {
    events: PollEvents,
    pipe: Arc<PipeInode>,
}

impl PipeWritePollFuture {
    fn new(pipe: Arc<PipeInode>, events: PollEvents) -> Self {
        Self { pipe, events }
    }
}

impl Future for PipeWritePollFuture {
    type Output = PollEvents;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.pipe.inner.lock();
        let mut res = PollEvents::empty();
        if self.events.contains(PollEvents::OUT) && !inner.buf.is_full() {
            res |= PollEvents::OUT;
            Poll::Ready(res)
        } else {
            if inner.is_read_closed {
                res |= PollEvents::ERR;
                return Poll::Ready(res);
            }
            inner.write_waker.push_back(cx.waker().clone());
            Poll::Pending
        }
    }
}

pub struct PipeWriteFile {
    meta: FileMeta,
}

impl PipeWriteFile {
    pub fn new(inode: Arc<PipeInode>) -> Arc<Self> {
        let meta = FileMeta::new(arc_zero(), inode);
        Arc::new(Self { meta })
    }
}

// NOTE: `PipeReadFile` is hold by task as `Arc<dyn File>`.
impl Drop for PipeWriteFile {
    fn drop(&mut self) {
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .unwrap_or_else(|_| unreachable!());
        log::debug!("[PipeWriteFile::drop] pipe write is closed");
        pipe.inner.lock().is_write_closed = true;
    }
}

pub struct PipeReadFile {
    meta: FileMeta,
}

impl PipeReadFile {
    pub fn new(inode: Arc<PipeInode>) -> Arc<Self> {
        let meta = FileMeta::new(arc_zero(), inode);
        Arc::new(Self { meta })
    }
}

impl Drop for PipeReadFile {
    fn drop(&mut self) {
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .unwrap_or_else(|_| unreachable!());
        log::debug!("[PipeReadFile::drop] pipe read is closed");
        pipe.inner.lock().is_read_closed = true;
    }
}

#[async_trait]
impl File for PipeWriteFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, _offset: usize, _buf: &mut [u8]) -> SysResult<usize> {
        Err(SysError::EBADF)
    }

    async fn base_write_at(&self, _offset: usize, buf: &[u8]) -> SysResult<usize> {
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .unwrap_or_else(|_| unreachable!());

        let revents = PipeWritePollFuture::new(pipe.clone(), PollEvents::OUT).await;
        if revents.contains(PollEvents::ERR) {
            return Err(SysError::EPIPE);
        }
        if revents.contains(PollEvents::OUT) {
            let mut inner = pipe.inner.lock();
            let space_left = inner.buf.capacity() - inner.buf.len();

            let len = cmp::min(space_left, buf.len());
            for i in 0..len {
                inner.buf.push(buf[i]);
            }
            if let Some(waker) = inner.read_waker.pop_front() {
                waker.wake();
            }
            log::trace!("[Pipe::write] already write buf {buf:?} with data len {len:?}");
            return Ok(len);
        }
        unreachable!()
    }

    async fn base_poll(&self, events: PollEvents) -> PollEvents {
        let waker = get_waker().await;
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .unwrap_or_else(|_| unreachable!());
        let mut inner = pipe.inner.lock();
        let mut res = PollEvents::empty();
        if events.contains(PollEvents::OUT) && !inner.buf.is_full() {
            res |= PollEvents::OUT;
        } else if inner.is_read_closed {
            res |= PollEvents::ERR;
        } else {
            inner.write_waker.push_back(waker);
        }
        res
    }
}

struct PipeReadPollFuture {
    events: PollEvents,
    pipe: Arc<PipeInode>,
}

impl PipeReadPollFuture {
    fn new(pipe: Arc<PipeInode>, events: PollEvents) -> Self {
        Self { pipe, events }
    }
}

impl Future for PipeReadPollFuture {
    type Output = PollEvents;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.pipe.inner.lock();
        let mut res = PollEvents::empty();
        if self.events.contains(PollEvents::IN) && !inner.buf.is_empty() {
            res |= PollEvents::IN;
            Poll::Ready(res)
        } else {
            if inner.is_write_closed {
                res |= PollEvents::HUP;
                return Poll::Ready(res);
            }
            inner.read_waker.push_back(cx.waker().clone());
            Poll::Pending
        }
    }
}

#[async_trait]
impl File for PipeReadFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, _offset: usize, buf: &mut [u8]) -> SysResult<usize> {
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .unwrap_or_else(|_| unreachable!());
        let events = PollEvents::IN;
        let revents = PipeReadPollFuture::new(pipe.clone(), events).await;

        if revents.contains(PollEvents::HUP) {
            return Ok(0);
        }
        if revents.contains(PollEvents::IN) {
            let mut inner = pipe.inner.lock();
            let len = core::cmp::min(inner.buf.len(), buf.len());
            for i in 0..len {
                buf[i] = inner
                    .buf
                    .dequeue()
                    .expect("Just checked for len, should not fail");
            }
            if let Some(waker) = inner.write_waker.pop_front() {
                waker.wake();
            }
            return Ok(len);
        }

        unreachable!()
    }

    async fn base_write_at(&self, _offset: usize, _buf: &[u8]) -> SysResult<usize> {
        Err(SysError::EBADF)
    }

    async fn base_poll(&self, events: PollEvents) -> PollEvents {
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .unwrap_or_else(|_| unreachable!());
        let waker = get_waker().await;
        let mut inner = pipe.inner.lock();
        let mut res = PollEvents::empty();
        if events.contains(PollEvents::IN) && !inner.buf.is_empty() {
            res |= PollEvents::IN;
        } else if inner.is_write_closed {
            res |= PollEvents::HUP;
            Poll::Ready(res);
        } else {
            inner.read_waker.push_back(waker);
        }
        res
    }
}

pub fn new_pipe() -> (Arc<dyn File>, Arc<dyn File>) {
    let pipe_inode = PipeInode::new();
    let read_end = PipeReadFile::new(pipe_inode.clone());
    let write_end = PipeWriteFile::new(pipe_inode);
    (read_end, write_end)
}
