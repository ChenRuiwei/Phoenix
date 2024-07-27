//! /Dev/random is a pseudo-random number generator device file

use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;
use config::board::BLOCK_SIZE;
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{
    Dentry, DentryMeta, DirEntry, File, FileMeta, Inode, InodeMeta, InodeMode, Stat, SuperBlock,
};

/// Linear congruence generator (LCG)
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    // 使用时间初始化种子
    pub const fn new() -> Self {
        // let seed = get_time_duration();
        let seed = 42;
        Self { state: seed }
    }

    // 生成下一个随机数
    pub fn next_u32(&mut self) -> u32 {
        const A: u64 = 6364136223846793005;
        const C: u64 = 1;
        self.state = self.state.wrapping_mul(A).wrapping_add(C);
        (self.state >> 32) as u32
    }

    #[allow(dead_code)]
    pub fn next_u8(&mut self) -> u8 {
        // LCG 参数：乘数、增量和模数
        const A: u64 = 1664525;
        const C: u64 = 1013904223;

        // 更新内部状态
        self.state = self.state.wrapping_mul(A).wrapping_add(C);

        // 返回最低 8 位
        (self.state >> 24) as u8
    }

    /// Generate a random number of u32 (4 bytes) at a time, and then split it
    /// into bytes to fill in the buf
    pub fn fill_buf(&mut self, buf: &mut [u8]) {
        let mut remaining = buf.len();
        let mut offset = 0;

        while remaining > 0 {
            // 生成一个随机的 u32 值
            let rand = self.next_u32();
            let rand_bytes = rand.to_le_bytes();

            // 计算要复制的字节数
            let chunk_size = remaining.min(4);

            // 将 rand_bytes 中的字节填充到 buf 中
            buf[offset..offset + chunk_size].copy_from_slice(&rand_bytes[..chunk_size]);

            // 更新剩余字节数和偏移量
            remaining -= chunk_size;
            offset += chunk_size;
        }
    }
}

pub struct UrandomDentry {
    meta: DentryMeta,
}

static mut RNG: SimpleRng = SimpleRng::new();

impl UrandomDentry {
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

impl Dentry for UrandomDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(Arc::new(UrandomFile {
            meta: FileMeta::new(self.clone(), self.inode()?),
        }))
    }

    fn base_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_create(self: Arc<Self>, name: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_unlink(self: Arc<Self>, name: &str) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }
}

pub struct UrandomInode {
    meta: InodeMeta,
}

impl UrandomInode {
    pub fn new(super_block: Arc<dyn SuperBlock>) -> Arc<Self> {
        let size = BLOCK_SIZE;
        Arc::new(Self {
            // accroding to linux, it should be S_IFCHR
            meta: InodeMeta::new(InodeMode::CHAR, super_block, size),
        })
    }
}

impl Inode for UrandomInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        let mode = self.meta.mode.bits();
        let len = inner.size;
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: mode,
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: len as u64,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: (len / 512) as u64,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }
}

pub struct UrandomFile {
    meta: FileMeta,
}

#[async_trait]
impl File for UrandomFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, _offset: usize, _buf: &mut [u8]) -> SyscallResult {
        unreachable!()
    }

    async fn base_write_at(&self, _offset: usize, _buf: &[u8]) -> SyscallResult {
        unreachable!()
    }

    async fn read_at(&self, _offset: usize, buf: &mut [u8]) -> SyscallResult {
        unsafe { RNG.fill_buf(buf) };
        Ok(buf.len())
    }

    async fn write_at(&self, _offset: usize, buf: &[u8]) -> SyscallResult {
        log::error!("[UrandomFile::base_write_at] does nothing");
        Ok(buf.len())
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }
}
