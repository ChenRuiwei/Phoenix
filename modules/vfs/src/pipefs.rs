use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;
use async_utils::yield_now;
use config::fs::PIPE_BUF_CAPACITY;
use ringbuffer::{AllocRingBuffer, RingBuffer};
use sync::mutex::SpinNoIrqLock;
use systype::SysError;
use vfs_core::{arc_zero, File, FileMeta, FileSystemType, Inode, InodeMeta, InodeMode};

type Mutex<T> = SpinNoIrqLock<T>;

pub struct PipeInode {
    meta: InodeMeta,
    is_closed: Mutex<bool>,
    buf: Mutex<AllocRingBuffer<u8>>,
}

impl PipeInode {
    pub fn new() -> Arc<Self> {
        let meta = InodeMeta::new(
            InodeMode::FIFO,
            Arc::<usize>::new_uninit(),
            PIPE_BUF_CAPACITY,
        );
        let buf = Mutex::new(AllocRingBuffer::new(PIPE_BUF_CAPACITY));
        Arc::new(Self {
            meta,
            is_closed: Mutex::new(false),
            buf,
        })
    }
}

impl Inode for PipeInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> systype::SysResult<vfs_core::Stat> {
        todo!()
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

impl Drop for PipeWriteFile {
    fn drop(&mut self) {
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .map_err(|_| SysError::EIO)
            .unwrap();
        *pipe.is_closed.lock() = true;
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

#[async_trait]
impl File for PipeWriteFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, _offset: usize, _buf: &mut [u8]) -> systype::SysResult<usize> {
        todo!()
    }

    async fn base_write(&self, _offset: usize, buf: &[u8]) -> systype::SysResult<usize> {
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .map_err(|_| SysError::EIO)?;
        let mut pipe_buf = pipe.buf.lock();
        let space_left = pipe_buf.capacity() - pipe_buf.len();

        let len = core::cmp::min(space_left, buf.len());
        for i in 0..len {
            pipe_buf.push(buf[i]);
        }
        log::trace!("[Pipe::write] already write buf {buf:?} with data len {len:?}");
        Ok(len)
    }

    fn base_read_dir(&self) -> systype::SysResult<Option<vfs_core::DirEntry>> {
        todo!()
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }
}

#[async_trait]
impl File for PipeReadFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, _offset: usize, buf: &mut [u8]) -> systype::SysResult<usize> {
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .map_err(|_| SysError::EIO)?;
        let mut pipe_len = pipe.buf.lock().len();
        while pipe_len == 0 {
            yield_now().await;
            pipe_len = pipe.buf.lock().len();
            if *pipe.is_closed.lock() {
                break;
            }
        }
        let mut pipe_buf = pipe.buf.lock();
        let len = core::cmp::min(pipe_buf.len(), buf.len());
        for i in 0..len {
            buf[i] = pipe_buf
                .dequeue()
                .expect("Just checked for len, should not fail");
        }
        Ok(len)
    }

    async fn base_write(&self, _offset: usize, _buf: &[u8]) -> systype::SysResult<usize> {
        todo!()
    }

    fn base_read_dir(&self) -> systype::SysResult<Option<vfs_core::DirEntry>> {
        todo!()
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }
}

pub fn new_pipe() -> (Arc<dyn File>, Arc<dyn File>) {
    let pipe_inode = PipeInode::new();
    let read_end = PipeReadFile::new(pipe_inode.clone());
    let write_end = PipeWriteFile::new(pipe_inode);
    (read_end, write_end)
}
