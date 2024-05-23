# FAT32 文件系统

FAT32，全称为 File Allocation Table 32，是一种文件系统格式，用于在各种存储设备上存储和管理文件和目录。它是 FAT 文件系统的一个版本，最初由微软在 1996 年引入，主要是为了解决 FAT16 在处理大容量存储设备时的限制问题。FAT32 文件系统在 Windows 操作系统以及许多其他设备和媒体中得到了广泛应用。

FAT32 的主要特点包括：

- 稳定性和兼容性：FAT32 提供了良好的稳定性和兼容性，能够兼容 Win 9X 及以前版本的 Windows 操作系统。
- 簇大小：使用比 FAT16 更小的簇（数据存储单元），从而提高了大容量硬盘上的空间利用率。
- 分区容量：支持的每个分区容量最大可达到 128TB，远大于 FAT16 的限制。
- 文件大小限制：单个文件最大支持 4GB，这对于处理大型文件来说是一个限制。

FAT32 文件系统的结构主要包括三个部分：

- 引导区：包含文件系统的具体信息，如 FAT 表个数、每个 FAT 表的大小、每扇区内的字节数目等。
- 文件分配表区：管理磁盘空间和文件，保存逻辑盘数据区各簇使用情况信息。
- 数据区：存放用户数据，以簇为分配单位来使用。

作为为 Windows 设计的文件系统，FAT32 并没有采取 UNIX 系列文件系统的设计范式。相比于 UNIX 系列的文件系统，FAT32 缺少 UNIX 规定的 `rwx` 权限管理，也没有提供硬链接功能或可以实现硬链接功能的模块。虽然要使内核支持 FAT32，只需实现对应的 VFS 接口，但是具体实现仍需要采取一些特殊机制。

Phoenix 使用了开源的 fatfs 库，并实现 FAT32 的 VFS 层操作来进行 FAT32 的对接。下面介绍 FAT32 对 VFS 的实现情况

## FAT32 实现 VFS

### FatSuperBlock

FAT32 文件系统的超级块定义如下：

```rust
pub struct FatSuperBlock {
    meta: SuperBlockMeta,
    /// Contain disk cursor, kernel time
    fs: Arc<FatFs>,
}
```

`FatSuperBlock` 自身实现方法如下：

```rust
impl FatSuperBlock {
    /// Use api from fatfs to initialize FatSuperBlock
    pub fn new(meta: SuperBlockMeta) -> Arc<Self> {
        let blk_dev = meta.device.as_ref().unwrap().clone();
        Arc::new(Self {
            meta,
            fs: Arc::new(
                FatFs::new(
                    DiskCursor {
                        sector: 0,
                        offset: 0,
                        blk_dev,
                    },
                    fatfs::FsOptions::new(),
                )
                .unwrap(),
            ),
        })
    }
}
```

`FatSuperBlock` 实现 `SuperBlock` 接口如下：

```rust
impl SuperBlock for FatSuperBlock {
    fn meta(&self) -> &SuperBlockMeta {
        &self.meta
    }

    fn stat_fs(&self) -> systype::SysResult<vfs_core::StatFs> {
        let stat_fs = self.fs.stats().map_err(as_sys_err)?;
        let ft = self.fs.fat_type();
        let f_type = match ft {
            fatfs::FatType::Fat12 => 0x01,
            fatfs::FatType::Fat16 => 0x04,
            fatfs::FatType::Fat32 => 0x0c,
        };
        Ok(StatFs {
            f_type,
            f_bsize: stat_fs.cluster_size() as i64,
            f_blocks: stat_fs.total_clusters() as u64,
            f_bfree: stat_fs.free_clusters() as u64,
            f_bavail: stat_fs.free_clusters() as u64,
            f_files: 0,
            f_ffree: 0,
            f_fsid: [0, 0],
            f_namelen: 255,
            f_frsize: 0,
            f_flags: 0,
            f_spare: [0; 4],
        })
    }
}
```

`FatSuperBlock` 实现了 `stat_fs` 方法，该方法返回的文件系统属性结构体定义如下

```rust
pub struct StatFs {
    /// magic number for indicating a file system
    pub f_type: i64,
    /// best size of a transporting block
    pub f_bsize: i64,
    /// number of blocks
    pub f_blocks: u64,
    /// number of free blocks
    pub f_bfree: u64,
    /// number of available blocks for users
    pub f_bavail: u64,
    /// total number of inodes
    pub f_files: u64,
    /// number of free inodes
    pub f_ffree: u64,
    /// file system id
    pub f_fsid: [i32; 2],
    /// max length of file name
    pub f_namelen: isize,
    /// size of fregment
    pub f_frsize: isize,
    /// some options
    pub f_flags: isize,
    /// padding
    pub f_spare: [isize; 4],
}
```

### Fat Inode

FAT32 文件系统将磁盘上存储数据的区域划分为簇，使用文件分配表记录文件的簇链信息，将文件名等信息存储在目录项中，没有显式地定义索引节点结构。基于上述特性，fatfs 对 FAT32 下的文件和文件夹分别进行了设计，因此 Phoenix 也分别实现了文件索引节点和文件夹索引节点两个结构体

#### FatFileInode

`FatFileInode` 的结构定义如下：

```rust
pub struct FatFileInode {
    meta: InodeMeta,
    /// file defined by fatfs
    pub file: Shared<FatFile>,
}
```

`FatFileInode` 实现的方法如下：

```rust
impl FatFileInode {
    /// Initialize a inode for fat
    pub fn new(super_block: Arc<dyn SuperBlock>, file: FatFile) -> Arc<Self> {
        let size = file.size().unwrap().try_into().unwrap();
        let inode = Arc::new(Self {
            meta: InodeMeta::new(
                InodeMode::from_type(InodeType::File),
                super_block.clone(),
                size,
            ),
            file: Arc::new(Mutex::new(file)),
        });
        super_block.push_inode(inode.clone());
        inode
    }
}
```

`FatFileInode` 对 `Inode` 接口的实现如下：

```rust
impl Inode for FatFileInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> systype::SysResult<Stat> {
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
```

`FatFileInode` 实现了 `get_attr` 方法，以支持内核的 `fstat` 系统调用。使用到的 `Stat` 结构体定义如下

```rust
pub struct Stat {
    /// device that the file system is mounted to
    pub st_dev: u64,
    /// inode number
    pub st_ino: u64,
    /// file type
    pub st_mode: u32,
    /// number of hard link
    pub st_nlink: u32,
    /// user id
    pub st_uid: u32,
    /// group id
    pub st_gid: u32,
    /// device number for real device, like char device.
    /// For regular files that are restored on HDD or SSD, st_rdev is usually defined as 0.
    pub st_rdev: u64,
    /// padding
    pub __pad: u64,
    /// file size
    pub st_size: u64,
    /// block size
    pub st_blksize: u32,
    /// padding
    pub __pad2: u32,
    /// number of blocks that are assigned to the file
    pub st_blocks: u64,
    pub st_atime: TimeSpec,
    pub st_mtime: TimeSpec,
    pub st_ctime: TimeSpec,
    pub unused: u64,
}
```
