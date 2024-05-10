# 虚拟文件系统

虚拟文件系统(Virtual File System，简称VFS)是内核中负责与各种字符流(如磁盘文件，IO设备等等)对接，并对外提供操作接口的子系统。
它为用户程序提供了一个统一的文件和文件系统操作接口，屏蔽了不同文件系统之间的差异和操作细节。
这意味着，用户程序可以使用标准的系统调用，如 `open()`、`read()`、`write()` 来操作文件，
而无需关心文件实际存储在哪种类型的文件系统或存储介质上。

Phoenix OS的虚拟文件系统设计以Linux为师，并利用Rust语言的特性，从面向对象的角度出发对虚拟文件系统进行了设计和优化。
目前虚拟文件系统包含 `SuperBlock`, `Inode`, `Dentry`, `File`等核心数据结构，也包含 `FdTable`, `Pipe`等用于实现系统调用的辅助数据结构，
可以支持包括 `sys_dup`, `sys_read` 在内的所有文件相关的系统调用

## 虚拟文件系统结构

## 核心数据结构及其操作

### SuperBlock

```rust
pub trait SuperBlock: Send + Sync {
    /// Get metadata of this super block.
    fn meta(&self) -> &SuperBlockMeta;

    /// Get filesystem statistics.
    fn stat_fs(&self) -> SysResult<StatFs>;

    /// Called when VFS is writing out all dirty data associated with a
    /// superblock.
    fn sync_fs(&self, wait: isize) -> SysResult<()>;

    fn set_root_dentry(&self, root_dentry: Arc<dyn Dentry>) {
        self.meta().root_dentry.call_once(|| root_dentry);
    }
}

impl dyn SuperBlock {
    /// Get the file system type of this super block.
    pub fn fs_type(&self) -> Arc<dyn FileSystemType> {
        self.meta().fs_type.upgrade().unwrap()
    }

    /// Get the root dentry.
    pub fn root_dentry(&self) -> Arc<dyn Dentry> {
        self.meta().root_dentry.get().unwrap().clone()
    }

    pub fn push_inode(&self, inode: Arc<dyn Inode>) {
        self.meta().inodes.lock().push(inode)
    }
}
```

### Inode

#### Inode Trait

#### InodeMeta

### Dentry

#### Dentry Trait

#### DentryMeta

### File

#### File Trait

#### FileMeta

### FileSystemType

#### FileSystemType Trait

#### FileSystemTypeMeta

### Path

## 辅助数据结构及其操作

### FdTable

### Pipe

## 虚拟文件系统层面的性能优化

## 相关系统调用及实现

### `sys_getcwd`

### `sys_pipe2`

### `sys_dup`

### `sys_dup3`

### `sys_chdir`

### `sys_openat`

### `sys_close`

### `sys_getdents64`

### `sys_read`

### `sys_write`

### `sys_linkat`

### `sys_unlinkat`

### `sys_mkdirat`

### `sys_unmount2`

### `sys_mount`

### `sys_fstat`
