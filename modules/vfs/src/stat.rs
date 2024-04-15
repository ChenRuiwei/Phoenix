use crate::FileSystemType;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Stat {
    // fs类型
    pub fs_type: FileSystemType,
    // 最优IO块大小
    pub fs_io_block_size: i64,
    // 总块数
    pub fs_blocks: u64,
    // 未分配块数
    pub fs_blocks_free: u64,
    // 用户视角下可用块数
    pub fs_blocks_avail: u64,
    // 总inode数，也是总文件数
    pub fs_inodes: u64,
    // 空闲inode数
    pub fs_inodes_free: u64,
    // 文件名长度限制
    pub fs_name_max_len: isize,
}
