# 虚拟文件系统

虚拟文件系统(Virtual File System，简称VFS)是内核中负责与各种字符流(如磁盘文件，IO设备等等)对接，并对外提供操作接口的子系统。它为用户程序提供了一个统一的文件和文件系统操作接口，屏蔽了不同文件系统之间的差异和操作细节。这意味着，用户程序可以使用标准的系统调用，如 `open()`、`read()`、`write()` 来操作文件，而无需关心文件实际存储在哪种类型的文件系统或存储介质上。

Phoenix OS的虚拟文件系统设计以Linux为师，并利用Rust语言的特性，从面向对象的角度出发对虚拟文件系统进行了设计和优化。目前可以支持包括 `sys_dup`, `sys_read` 在内的所有文件相关的系统调用

## 虚拟文件系统结构

目前虚拟文件系统包含 `SuperBlock`, `Inode`, `Dentry`, `File`等核心数据结构, 也包含 `FdTable`, `Pipe`等用于实现系统调用的辅助数据结构。

## 核心数据结构及其操作

### SuperBlock

超级块对象用于存储和管理特定文件系统的信息，一个文件系统实例对应一个超级块。每当一个文件系统挂载到操作系统，内核需要调用相应函数创建该文件系统的超级块。当文件系统卸载时，也需要删除相应超级块。

超级块的操作由 `SuperBlock` 表示，其形式如下：

```rust
pub trait SuperBlock: Send + Sync {
    /// Get metadata of this super block.
    fn meta(&self) -> &SuperBlockMeta;

    /// Get filesystem statistics.
    fn stat_fs(&self) -> SysResult<StatFs>;

    /// Called when VFS is writing out all dirty data associated with a
    /// superblock.
    fn sync_fs(&self, wait: isize) -> SysResult<()>;

    /// Set the root dentry of this super block to root_dentry
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

    /// Called when an inode is created
    pub fn push_inode(&self, inode: Arc<dyn Inode>) {
        self.meta().inodes.lock().push(inode)
    }
}
```

超级块对象的设计由 `SuperBlockMeta` 结构体表示，下面给出它的结构和描述：

```rust
pub struct SuperBlockMeta {
    /// Block device that hold this file system.
    pub device: Option<Arc<dyn BlockDevice>>,
    /// File system type.
    pub fs_type: Weak<dyn FileSystemType>,
    /// Root dentry points to the mount point.
    pub root_dentry: Once<Arc<dyn Dentry>>,
    /// All inodes.
    pub inodes: Mutex<Vec<Arc<dyn Inode>>>,
    /// All dirty inodes.
    pub dirty: Mutex<Vec<Arc<dyn Inode>>>,
}
```

### Inode

索引节点负责管理在文件系统中文件的信息。对于文件系统中的文件来说，文件名是可以更改的，也可以不唯一，但是文件的索引节点一定是唯一的，并且该文件的所有文件名在进行路径查找时都会查找到相同的索引节点。

索引节点的操作由 `Inode` 表示，可以进行的操作如下：

```rust
pub trait Inode: Send + Sync + DowncastSync {
    /// Get metadata of this Inode
    fn meta(&self) -> &InodeMeta;

    /// Get attributes of this file
    fn get_attr(&self) -> SysResult<Stat>;
}

impl dyn Inode {
    /// Get inode number of this inode
    pub fn ino(&self) -> usize {
        self.meta().ino
    }

    /// Get file's type, i.e. File, Dir, Socket
    pub fn itype(&self) -> InodeType {
        self.meta().mode.to_type()
    }

    /// Get size of this file
    pub fn size(&self) -> usize {
        self.meta().inner.lock().size
    }

    /// Set the size for this file
    pub fn set_size(&self, size: usize) {
        self.meta().inner.lock().size = size;
    }

    /// Get state of this file, i.e. Init, Sync, Dirty
    pub fn state(&self) -> InodeState {
        self.meta().inner.lock().state
    }

    /// Set state for this file
    pub fn set_state(&self, state: InodeState) {
        self.meta().inner.lock().state = state;
    }
}
```

索引节点对象由 `InodeMeta` 结构体表示，下面给出它的结构和描述：

```rust
pub struct InodeMeta {
    /// Inode number.
    pub ino: usize,
    /// mode of inode.
    pub mode: InodeMode,
    /// Super block this inode belongs to
    pub super_block: Weak<dyn SuperBlock>,
    /// Data in inner may be altered, so cover it in mutex form
    pub inner: Mutex<InodeMetaInner>,
}

pub struct InodeMetaInner {
    /// Size of a file in bytes.
    pub size: usize,
    /// Last access time.
    pub atime: TimeSpec,
    /// Last modification time.
    pub mtime: TimeSpec,
    /// Last status change time.
    pub ctime: TimeSpec,
    /// State of a file
    pub state: InodeState,
}
```

### Dentry

目录项是管理文件在目录树中的信息的结构体。在文件系统中，以挂载点，即文件系统的根目录为根节点，按照文件夹层级往下，整个文件系统是一个目录树的结构，树的中间节点为文件夹，树的叶节点为普通文件。目录树中的一个节点，对应文件系统中的一个目录项。每一个目录项都指向一个文件的索引节点，同时不同的目录项可以指向相同的索引节点（即硬链接）。

目录项的操作由 `Dentry` 描述，其形式如下：

```rust
pub trait Dentry: Send + Sync {
    /// Get metadata of this Dentry
    fn meta(&self) -> &DentryMeta;

    /// Open a file associated with the inode that this dentry points to.
    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>>;

    /// Look up in a directory inode and find file with `name`.
    ///
    /// If the named inode does not exist, a negative dentry will be created as
    /// a child and returned. Returning an error code from this routine must
    /// only be done on a real error.
    fn base_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>>;

    /// Called by the open(2) and creat(2) system calls. Create an inode for a
    /// dentry in the directory inode.
    ///
    /// If the dentry itself has a negative child with `name`, it will create an
    /// inode for the negative child and return the child.
    fn base_create(self: Arc<Self>, name: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>>;

    /// Called by the unlink(2) system call. Delete a file inode in a directory
    /// inode.
    fn base_unlink(self: Arc<Self>, name: &str) -> SyscallResult;

    /// Called by the rmdir(2) system call. Delete a dir inode in a directory
    /// inode.
    fn base_rmdir(self: Arc<Self>, name: &str) -> SyscallResult;

    /// Get child dentry of this dentry. If not, create a new child dentry
    fn get_child_or_create(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        self.get_child(name).unwrap_or_else(|| {
            let new_dentry = self.clone().base_new_child(name);
            self.insert(new_dentry.clone());
            new_dentry
        })
    }

    /// Get inode that this dentry points to
    fn inode(&self) -> SysResult<Arc<dyn Inode>> {
        self.meta()
            .inode
            .lock()
            .as_ref()
            .ok_or(SysError::ENOENT)
            .cloned()
    }

    /// Get super block that this dentry belongs to
    fn super_block(&self) -> Arc<dyn SuperBlock> {
        self.meta().super_block.upgrade().unwrap()
    }

    /// Get name of this dentry in the form of String
    fn name_string(&self) -> String {
        self.meta().name.clone()
    }

    /// Get name of this dentry
    fn name(&self) -> &str {
        &self.meta().name
    }

    /// Get parent dentry of this dentry
    fn parent(&self) -> Option<Arc<dyn Dentry>> {
        self.meta().parent.as_ref().map(|p| p.upgrade().unwrap())
    }

    /// Get children dentries of this dentry, which is returned in Map
    fn children(&self) -> BTreeMap<String, Arc<dyn Dentry>> {
        self.meta().children.lock().clone()
    }

    /// Get child dentry according to name
    fn get_child(&self, name: &str) -> Option<Arc<dyn Dentry>> {
        self.meta().children.lock().get(name).cloned()
    }

    /// Set inode for this dentry
    fn set_inode(&self, inode: Arc<dyn Inode>) {
        if self.meta().inode.lock().is_some() {
            log::warn!("[Dentry::set_inode] replace inode in {:?}", self.name());
        }
        *self.meta().inode.lock() = Some(inode);
    }

    /// Insert a child dentry to this dentry.
    fn insert(&self, child: Arc<dyn Dentry>) -> Option<Arc<dyn Dentry>> {
        self.meta()
            .children
            .lock()
            .insert(child.name_string(), child)
    }

    /// Get the path of this dentry.
    fn path(&self) -> String {
        if let Some(p) = self.parent() {
            let path = if self.name() == "/" {
                String::from("")
            } else {
                String::from("/") + self.name()
            };
            let parent_name = p.name();
            return if parent_name == "/" {
                if p.parent().is_some() {
                    // p is a mount point
                    p.parent().unwrap().path() + path.as_str()
                } else {
                    path
                }
            } else {
                // p is not root
                p.path() + path.as_str()
            };
        } else {
            log::warn!("dentry has no parent");
            String::from("/")
        }
    }
}

impl dyn Dentry {
    /// Get whether this dentry is negative or not
    pub fn is_negetive(&self) -> bool {
        self.meta().inode.lock().is_none()
    }

    /// Turn this dentry to negative
    pub fn clear_inode(&self) {
        *self.meta().inode.lock() = None;
    }

    /// Remove a child from this dentry and return the child.
    pub fn remove(&self, name: &str) -> Option<Arc<dyn Dentry>> {
        self.meta().children.lock().remove(name)
    }

    /// Open a file associated with this dentry
    pub fn open(self: &Arc<Self>) -> SysResult<Arc<dyn File>> {
        self.clone().base_open()
    }

    /// Loop up a dentry given its name
    pub fn lookup(self: &Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        let child = self.get_child(name);
        if child.is_some() {
            return Ok(child.unwrap());
        }
        self.clone().base_lookup(name)
    }

    /// create an inode for a dentry in a directory inode
    pub fn create(self: &Arc<Self>, name: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        self.clone().base_create(name, mode)
    }

    /// Delete a file inode in a directory inode
    pub fn unlink(self: &Arc<Self>, name: &str) -> SyscallResult {
        self.clone().base_unlink(name)
    }

    /// Delete a dir inode in a directory inode
    pub fn rmdir(self: &Arc<Self>, name: &str) -> SyscallResult {
        self.clone().base_rmdir(name)
    }
}
```

目录项对象由 `DentryMeta` 结构体表示，下面给出它的结构和描述：

```rust
pub struct DentryMeta {
    /// Name of this file or directory.
    pub name: String,
    /// Super block this dentry belongs to
    pub super_block: Weak<dyn SuperBlock>,
    /// Parent dentry. `None` if root dentry.
    pub parent: Option<Weak<dyn Dentry>>,
    /// Inode it points to. May be `None`, which is called negative dentry.
    pub inode: Mutex<Option<Arc<dyn Inode>>>,
    /// Children dentries. Key value pair is <name, dentry>.
    pub children: Mutex<BTreeMap<String, Arc<dyn Dentry>>>,
}
```

### File

文件对象是进程已打开的文件在内存中的表示。文件对象由系统调用 `open()` 创建，由系统调用 `close()` 撤销，所有文件相关的系统调用实际上都是文件对象定义的操作。文件对象与文件系统中的文件并不是一一对应的关系，因为多个进程可能会同时打开同一个文件，也就会创建多个文件对象，但这些文件对象指向的索引节点都是同一个索引节点，即同一个文件。

文件对象的操作由 `File` 描述，其形式如下：

```rust
pub trait File: Send + Sync {
    /// Get metadata of this file
    fn meta(&self) -> &FileMeta;

    /// Called by read(2) and related system calls.
    ///
    /// On success, the number of bytes read is returned (zero indicates end of
    /// file), and the file position is advanced by this number.
    async fn read(&self, offset: usize, buf: &mut [u8]) -> SyscallResult;

    /// Called by write(2) and related system calls.
    ///
    /// On success, the number of bytes written is returned, and the file offset
    /// is incremented by the number of bytes actually written.
    async fn write(&self, offset: usize, buf: &[u8]) -> SyscallResult;

    /// Read directory entries. This is called by the getdents(2) system call.
    ///
    /// For every call, this function will return an valid entry, or an error.
    /// If it read to the end of directory, it will return an empty entry.
    fn base_read_dir(&self) -> SysResult<Option<DirEntry>>;

    /// Called by the close(2) system call to flush a file
    fn flush(&self) -> SysResult<usize>;

    /// called by the ioctl(2) system call.
    fn ioctl(&self, cmd: usize, arg: usize) -> SyscallResult {
        Err(SysError::ENOTTY)
    }

    /// 
    async fn poll(&self, events: PollEvents) -> SysResult<PollEvents> {
        let mut res = PollEvents::empty();
        if events.contains(PollEvents::POLLIN) {
            res |= PollEvents::POLLIN;
        }
        if events.contains(PollEvents::POLLOUT) {
            res |= PollEvents::POLLOUT;
        }
        Ok(res)
    }

    fn inode(&self) -> Arc<dyn Inode> {
        self.meta().inode.clone()
    }

    // NOTE: super block has an arc of inode
    fn i_cnt(&self) -> usize {
        Arc::strong_count(&self.meta().inode)
    }

    fn itype(&self) -> InodeType {
        self.meta().inode.itype()
    }

    /// Called when the VFS needs to move the file position index.
    ///
    /// Return the result offset.
    fn seek(&self, pos: SeekFrom) -> SysResult<usize> {
        let mut res_pos = self.pos();
        match pos {
            SeekFrom::Current(off) => {
                if off < 0 {
                    res_pos -= off.abs() as usize;
                } else {
                    res_pos += off as usize;
                }
            }
            SeekFrom::Start(off) => {
                res_pos = off as usize;
            }
            SeekFrom::End(off) => {
                let size = self.size();
                if off < 0 {
                    res_pos = size - off.abs() as usize;
                } else {
                    res_pos = size + off as usize;
                }
            }
        }
        self.set_pos(res_pos);
        Ok(res_pos)
    }

    fn pos(&self) -> usize {
        self.meta().pos.load(Ordering::Relaxed)
    }

    fn set_pos(&self, pos: usize) {
        self.meta().pos.store(pos, Ordering::Relaxed)
    }

    fn dentry(&self) -> Arc<dyn Dentry> {
        self.meta().dentry.clone()
    }

    fn super_block(&self) -> Arc<dyn SuperBlock> {
        self.meta().dentry.super_block()
    }

    fn size(&self) -> usize {
        self.meta().inode.size()
    }
}

impl dyn File {
    pub fn flags(&self) -> OpenFlags {
        self.meta().flags.lock().clone()
    }

    pub fn set_flags(&self, flags: OpenFlags) {
        *self.meta().flags.lock() = flags;
    }

    pub fn load_dir(&self) -> SysResult<()> {
        let inode = self.inode();
        if inode.state() == InodeState::Init {
            self.base_load_dir()?;
            inode.set_state(InodeState::Synced)
        }
        Ok(())
    }

    pub fn read_dir(&self) -> SysResult<Option<DirEntry>> {
        self.load_dir()?;
        if let Some(sub_dentry) = self
            .dentry()
            .children()
            .values()
            .filter(|c| !c.is_negetive())
            .nth(self.pos())
        {
            self.seek(SeekFrom::Current(1))?;
            let inode = sub_dentry.inode()?;
            let dirent = DirEntry {
                ino: inode.ino() as u64,
                off: self.pos() as u64,
                itype: inode.itype(),
                name: sub_dentry.name_string(),
            };
            Ok(Some(dirent))
        } else {
            Ok(None)
        }
    }

    /// Read all data from this file synchronously.
    pub async fn read_all_from_start(&self, buffer: &mut Vec<u8>) -> SysResult<()> {
        let old_pos = self.seek(SeekFrom::Start(0_u64))?;
        buffer.clear();
        buffer.resize(PAGE_SIZE, 0);
        let mut idx = 0;
        loop {
            let len = self
                .read(idx, &mut buffer.as_mut_slice()[idx..idx + PAGE_SIZE])
                .await?;
            // log::trace!("[read_all_from_start] read len: {}", len);
            if len < PAGE_SIZE {
                break;
            }
            debug_assert_eq!(len, PAGE_SIZE);
            idx += len;
            buffer.resize(idx + PAGE_SIZE, 0);
            // log::trace!("[read_all_from_start] buf len: {}", buffer.len());
        }
        self.seek(SeekFrom::Start(old_pos as u64))?;
        Ok(())
    }
}
```

```rust
pub struct FileMeta {
    /// Dentry which pointes to this file.
    pub dentry: Arc<dyn Dentry>,
    pub inode: Arc<dyn Inode>,

    /// Offset position of this file.
    /// WARN: may cause trouble if this is not locked with other things.
    pub pos: AtomicUsize,
    pub flags: Mutex<OpenFlags>,
}
```

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
