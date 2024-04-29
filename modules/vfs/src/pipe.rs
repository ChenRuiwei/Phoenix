use alloc::sync::Arc;

use config::fs::PIPE_BUF_CAPACITY;
use ringbuffer::{AllocRingBuffer, RingBuffer};
use sync::mutex::SpinLock;
use systype::SysError;
use vfs_core::{arc_zero, File, FileMeta, Inode, InodeMeta, InodeMode};

type Mutex<T> = SpinLock<T>;

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

pub struct PipeReadFile {
    meta: FileMeta,
}

impl PipeReadFile {
    pub fn new(inode: Arc<PipeInode>) -> Arc<Self> {
        let meta = FileMeta::new(arc_zero(), inode);
        Arc::new(Self { meta })
    }
}

impl File for PipeWriteFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> systype::SysResult<usize> {
        todo!()
    }

    fn write(&self, offset: usize, buf: &[u8]) -> systype::SysResult<usize> {
        let mut pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .map_err(|_| SysError::EIO)?;
        let mut pipe_buf = pipe.buf.lock();
        let space_left = pipe_buf.capacity() - pipe_buf.len();

        // debug_assert_ne!(space_left, 0);
        // if buf.len() == 0 {
        //     // ensure we at least write one byte
        //     return Ok(0);
        // }

        let len = Ord::min(space_left, buf.len());
        for i in 0..len {
            pipe_buf.push(buf[i]);
        }

        log::trace!("[Pipe::write] buf {buf:?}");

        Ok(len)
    }

    fn read_dir(&self) -> systype::SysResult<Option<vfs_core::DirEntry>> {
        todo!()
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }
}

impl File for PipeReadFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> systype::SysResult<usize> {
        let pipe = self
            .inode()
            .downcast_arc::<PipeInode>()
            .map_err(|_| SysError::EIO)?;
        let mut pipe_buf = pipe.buf.lock();

        while pipe_buf.len() == 0 {
            core::hint::spin_loop();
        }

        // debug_assert_ne!(pipe_buf.len(), 0);
        // if buf.len() == 0 {
        //     // ensure we at least read one byte
        //     return Ok(0);
        // }

        let len = Ord::min(pipe_buf.len(), buf.len());
        for i in 0..len {
            buf[i] = pipe_buf
                .dequeue()
                .expect("Just checked for len, should not fail");
        }
        Ok(len)
    }

    fn write(&self, offset: usize, buf: &[u8]) -> systype::SysResult<usize> {
        todo!()
    }

    fn read_dir(&self) -> systype::SysResult<Option<vfs_core::DirEntry>> {
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
