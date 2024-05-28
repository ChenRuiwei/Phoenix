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

Phoenix 使用了开源的 rust-fatfs 库，并在其基础上添加了多核的支持。通过实现 FAT32 的 VFS 层接口完成了 FAT32 的对接。

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

### FAT Inode

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
    /// last access time
    pub st_atime: TimeSpec,
    /// last modification time
    pub st_mtime: TimeSpec,
    /// last change of file's meta data time
    pub st_ctime: TimeSpec,
    pub unused: u64,
}
```

#### FatDirInode

`FatDirInode` 结构体定义如下：

```rust
pub struct FatDirInode {
    meta: InodeMeta,
    pub dir: Shared<FatDir>,
}
```

`FatDirInode` 实现 `Inode` 情况如下：

```rust
impl Inode for FatDirInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> systype::SysResult<vfs_core::Stat> {
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

与 `FatFileInode` 相同，`FatDirInode` 实现了 `get_attr` 方法，用于 `fstat` 系统调用中。

### FatDentry

`FatDentry` 结构体定义如下：

```rust
pub struct FatDentry {
    meta: DentryMeta,
}
```

里面包含 `FatDentry` 的元数据

`FatDentry` 对 `Dentry` 的实现情况如下：

```rust
impl Dentry for FatDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> systype::SysResult<Arc<dyn vfs_core::File>> {
        let inode = self.inode()?;
        match inode.itype() {
            InodeType::File => {
                let inode = inode
                    .downcast_arc::<FatFileInode>()
                    .map_err(|_| SysError::EIO)?;
                Ok(FatFileFile::new(self.clone(), inode))
            }
            InodeType::Dir => {
                let inode = inode
                    .downcast_arc::<FatDirInode>()
                    .map_err(|_| SysError::EIO)?;
                Ok(FatDirFile::new(self.clone(), inode))
            }
            _ => Err(SysError::EPERM),
        }
    }

    fn base_lookup(self: Arc<Self>, name: &str) -> systype::SysResult<Arc<dyn Dentry>> {
        let sb = self.super_block();
        let inode = self
            .inode()?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        let find = inode.dir.lock().iter().find(|e| {
            let entry = e.as_ref().unwrap();
            let e_name = entry.file_name();
            name == e_name
        });
        let sub_dentry = self.into_dyn().get_child_or_create(name);
        if let Some(find) = find {
            log::debug!("[FatDentry::base_lookup] find name {name}");
            let entry = find.map_err(as_sys_err)?;
            let new_inode: Arc<dyn Inode> = if entry.is_dir() {
                let new_dir = entry.to_dir();
                FatDirInode::new(sb, new_dir)
            } else {
                let new_file = entry.to_file();
                FatFileInode::new(sb, new_file)
            };
            sub_dentry.set_inode(new_inode);
        } else {
            log::warn!("[FatDentry::base_lookup] name {name} does not exist");
        }
        Ok(sub_dentry)
    }

    fn base_create(
        self: Arc<Self>,
        name: &str,
        mode: vfs_core::InodeMode,
    ) -> systype::SysResult<Arc<dyn Dentry>> {
        log::trace!("[FatDentry::base_create] create name {name}, mode {mode:?}");
        let sb = self.super_block();
        let inode = self
            .inode()?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        let sub_dentry = self.into_dyn().get_child_or_create(name);
        match mode.to_type() {
            InodeType::Dir => {
                let new_dir = inode.dir.lock().create_dir(name).map_err(as_sys_err)?;
                let new_inode = FatDirInode::new(sb.clone(), new_dir);
                sub_dentry.set_inode(new_inode);
                Ok(sub_dentry)
            }
            InodeType::File => {
                let new_file = inode.dir.lock().create_file(name).map_err(as_sys_err)?;
                let new_inode = FatFileInode::new(sb.clone(), new_file);
                sub_dentry.set_inode(new_inode);
                Ok(sub_dentry)
            }
            _ => {
                log::warn!("[FatDentry::base_create] not supported mode {mode:?}");
                Err(SysError::EIO)
            }
        }
    }

    fn base_unlink(self: Arc<Self>, name: &str) -> systype::SyscallResult {
        let inode = self
            .inode()?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        let sub_dentry = self.get_child(name).ok_or(SysError::ENOENT)?;
        if sub_dentry.inode()?.itype().is_dir() {
            return Err(SysError::EISDIR);
        }
        sub_dentry.clear_inode();
        inode.dir.lock().remove(name).map_err(as_sys_err)?;
        Ok(0)
    }

    fn base_rmdir(self: Arc<Self>, name: &str) -> systype::SyscallResult {
        let inode = self
            .inode()?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        let sub_dentry = self.get_child(name).ok_or(SysError::ENOENT)?;
        if !sub_dentry.inode()?.itype().is_dir() {
            return Err(SysError::ENOTDIR);
        }
        sub_dentry.clear_inode();
        inode.dir.lock().remove(name).map_err(as_sys_err)?;
        Ok(0)
    }

    fn base_new_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        Self::new(name, self.super_block(), Some(self))
    }
}
```

从 `Dentry` 的实现中可见，`Dentry` 负责向外开放了大部分文件操作的接口，而接口的具体实现又是以 `Inode` 为核心。

### FAT File

#### FatFileFile

`FatFileFile` 的结构体定义如下：

```rust
pub struct FatFileFile {
    meta: FileMeta,
    file: Shared<FatFile>,
}
```

`FatFileFile` 对 `File` 的实现情况如下：

```rust
impl File for FatFileFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        match self.itype() {
            InodeType::File => {
                let mut file = self.file.lock();
                let fat_offset = file.offset() as usize;
                if offset != fat_offset {
                    file.seek(fatfs::SeekFrom::Start(offset as u64))
                        .map_err(as_sys_err)?;
                }
                let count = file.read(buf).map_err(as_sys_err)?;
                log::trace!("[FatFileFile::base_read] count {count}");
                Ok(count)
            }
            InodeType::Dir => Err(SysError::EISDIR),
            _ => unreachable!(),
        }
    }

    async fn base_write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        if buf.is_empty() {
            return Ok(0);
        }
        match self.itype() {
            InodeType::File => {
                let mut file = self.file.lock();
                let size = self.inode().size();
                if offset > size {
                    // write empty data to fill area [size, offset)
                    let empty = vec![0; offset - size];
                    file.seek(fatfs::SeekFrom::Start(size as u64))
                        .map_err(as_sys_err)?;
                    file.write_all(&empty).map_err(as_sys_err)?;
                }

                let fat_offset = file.offset() as usize;
                if offset != fat_offset {
                    file.seek(fatfs::SeekFrom::Start(offset as u64))
                        .map_err(as_sys_err)?;
                }
                file.write_all(buf).map_err(as_sys_err)?;
                if offset + buf.len() > size {
                    let new_size = offset + buf.len();
                    self.inode().set_size(new_size);
                }
                Ok(buf.len())
            }
            InodeType::Dir => Err(SysError::EISDIR),
            _ => unreachable!(),
        }
    }
}
```

#### FatDirFile

`FatDirFile` 的结构体定义如下：

```rust
pub struct FatDirFile {
    meta: FileMeta,
    dir: Shared<FatDir>,
    iter_cache: Shared<FatDirIter>,
}
```

`FatDirFile` 对 `File` 的实现情况如下：

```rust
impl File for FatDirFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, _offset: usize, _buf: &mut [u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    async fn base_write_at(&self, _offset: usize, _buf: &[u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    fn base_read_dir(&self) -> systype::SysResult<Option<vfs_core::DirEntry>> {
        let entry = self.iter_cache.lock().next();
        let Some(entry) = entry else {
            return Ok(None);
        };
        let Ok(entry) = entry else {
            return Err(SysError::EIO);
        };
        let name = entry.file_name();
        self.seek(SeekFrom::Current(1))?;
        let sub_dentry = self.dentry().get_child_or_create(&name);
        let new_inode: Arc<dyn Inode> = if entry.is_dir() {
            let new_dir = entry.to_dir();
            FatDirInode::new(self.super_block(), new_dir)
        } else {
            let new_file = entry.to_file();
            FatFileInode::new(self.super_block(), new_file)
        };
        let itype = new_inode.itype();
        sub_dentry.set_inode(new_inode);
        let entry = DirEntry {
            ino: 1,                 // Fat32 does not support ino on disk
            off: self.pos() as u64, // off should not be used
            itype,
            name,
        };
        Ok(Some(entry))
    }

    fn base_load_dir(&self) -> SysResult<()> {
        let mut iter = self.dir.lock().iter();
        while let Some(entry) = iter.next() {
            let Ok(entry) = entry else {
                return Err(SysError::EIO);
            };
            let name = entry.file_name();
            let sub_dentry = self.dentry().get_child_or_create(&name);
            let new_inode: Arc<dyn Inode> = if entry.is_dir() {
                let new_dir = entry.to_dir();
                FatDirInode::new(self.super_block(), new_dir)
            } else {
                let new_file = entry.to_file();
                FatFileInode::new(self.super_block(), new_file)
            };
            sub_dentry.set_inode(new_inode);
        }
        Ok(())
    }
}
```

### FatFsType

`FatFsType` 的结构体定义如下：

```rust
pub struct FatFsType {
    meta: FileSystemTypeMeta,
}
```

`FatFsType` 对 `FileSystemType` 的实现情况如下：

```rust
impl FileSystemType for FatFsType {
    fn meta(&self) -> &FileSystemTypeMeta {
        &self.meta
    }

    fn base_mount(
        self: Arc<Self>,
        name: &str,
        parent: Option<Arc<dyn Dentry>>,
        _flags: vfs_core::MountFlags,
        dev: Option<Arc<dyn driver::BlockDevice>>,
    ) -> systype::SysResult<Arc<dyn vfs_core::Dentry>> {
        debug_assert!(dev.is_some());
        let sb = FatSuperBlock::new(SuperBlockMeta::new(dev, self.clone()));
        let root_inode = FatDirInode::new(sb.clone(), sb.fs.root_dir());
        let root_dentry = FatDentry::new(name, sb.clone(), parent.clone()).into_dyn();
        root_dentry.set_inode(root_inode);
        if let Some(parent) = parent {
            parent.insert(root_dentry.clone());
        }
        sb.set_root_dentry(root_dentry.clone());
        self.insert_sb(&root_dentry.path(), sb);
        Ok(root_dentry)
    }
}
```
