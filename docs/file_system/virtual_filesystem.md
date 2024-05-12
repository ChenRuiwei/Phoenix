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

    /// called when a process wants to check if there is activity on this file and (optionally) 
    /// go to sleep until there is activity. 
    /// Called by the select(2) and poll(2) system calls
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

    /// Get the inode of this file
    fn inode(&self) -> Arc<dyn Inode> {
        self.meta().inode.clone()
    }

    
    /// Get the count of strong reference on inode
    fn i_cnt(&self) -> usize {
        Arc::strong_count(&self.meta().inode)
    }

    /// Get type of this file
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

    /// Get the file position index
    fn pos(&self) -> usize {
        self.meta().pos.load(Ordering::Relaxed)
    }

    /// Set file position index for this file
    fn set_pos(&self, pos: usize) {
        self.meta().pos.store(pos, Ordering::Relaxed)
    }

    /// Get dentry of this file
    fn dentry(&self) -> Arc<dyn Dentry> {
        self.meta().dentry.clone()
    }

    /// Get super block this file belongs to
    fn super_block(&self) -> Arc<dyn SuperBlock> {
        self.meta().dentry.super_block()
    }

    /// Get size of this file
    fn size(&self) -> usize {
        self.meta().inode.size()
    }
}

impl dyn File {
    /// Get file's opening mode, i.e. RDONLY, WRONLY, SYNC
    pub fn flags(&self) -> OpenFlags {
        self.meta().flags.lock().clone()
    }

    /// Set file's opening mode
    pub fn set_flags(&self, flags: OpenFlags) {
        *self.meta().flags.lock() = flags;
    }

    /// Called by directory to load all dentry and inodes from disk if it hasn't been done.
    pub fn load_dir(&self) -> SysResult<()> {
        let inode = self.inode();
        if inode.state() == InodeState::Init {
            self.base_load_dir()?;
            inode.set_state(InodeState::Synced)
        }
        Ok(())
    }

    /// Get child DirEntry in this directory inode that position index points to
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

文件对象的设计由 `FileMeta` 结构体表示，下面给出它的结构和描述：

```rust
pub struct FileMeta {
    /// Dentry which points to this file.
    pub dentry: Arc<dyn Dentry>,
    /// Inode which points to this file
    pub inode: Arc<dyn Inode>,
    /// Offset position of this file.
    pub pos: AtomicUsize,
    /// File mode
    pub flags: Mutex<OpenFlags>,
}
```

### FileSystemType

`FileSystemType` 用来描述各种特定文件系统类型的功能和行为，并负责管理每种文件系统下的所有文件系统实例以及对应的超级块。

`FileSystemType` 操作的形式如下：

```rust
pub trait FileSystemType: Send + Sync {
    fn meta(&self) -> &FileSystemTypeMeta;

    /// Call when a new instance of this filesystem should be mounted.
    fn arc_mount(
        self: Arc<Self>,
        abs_mount_path: &str,
        flags: MountFlags,
        dev: Option<Arc<dyn BlockDevice>>,
    ) -> SysResult<Arc<dyn Dentry>>;

    /// Call when an instance of this filesystem should be shut down.
    fn kill_sb(&self, sb: Arc<dyn SuperBlock>) -> SysResult<()>;

    /// Insert a super block into file system type
    fn insert_sb(&self, abs_mount_path: &str, super_block: Arc<dyn SuperBlock>) {
        self.meta()
            .supers
            .lock()
            .insert(abs_mount_path.to_string(), super_block);
    }

    /// Get the name of this file system type
    fn name(&self) -> &str {
        &self.meta().name
    }

    /// Get the name of this file system type in the form of String
    fn name_string(&self) -> String {
        self.meta().name.to_string()
    }
}

impl dyn FileSystemType {
    /// the method to call when a new instance of this filesystem should be mounted
    pub fn mount(
        self: &Arc<Self>,
        abs_mount_path: &str,
        flags: MountFlags,
        dev: Option<Arc<dyn BlockDevice>>,
    ) -> SysResult<Arc<dyn Dentry>> {
        self.clone().arc_mount(abs_mount_path, flags, dev)
    }

    /// Get the super block of a file system according to its mount path
    pub fn get_sb(&self, abs_mount_path: &str) -> SysResult<Arc<dyn SuperBlock>> {
        self.meta()
            .supers
            .lock()
            .get(abs_mount_path)
            .cloned()
            .ok_or(SysError::ENOENT)
    }
}
```

`FileSystemType`的设计由 `FileSystemTypeMeta` 结构体表示，下面给出它的结构和描述：

```rust
pub struct FileSystemTypeMeta {
    /// Name of this file system type.
    name: String,
    /// Super blocks.
    supers: Mutex<BTreeMap<String, Arc<dyn SuperBlock>>>,
}
```

### Path

`Path` 结构体的主要功能是管理和操作文件路径，实施便捷的路径查找。

`Path`的操作形式如下：

```rust
impl Path {
    /// Create a new path struct
    pub fn new(root: Arc<dyn Dentry>, start: Arc<dyn Dentry>, path: &str) -> Self {
        Self {
            root,
            start,
            path: path.to_string(),
        }
    }

    /// Walk until path has been resolved.
    pub fn walk(&self, mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        let path = self.path.as_str();
        let mut dentry = if is_absolute_path(path) {
            self.root.clone()
        } else {
            self.start.clone()
        };
        log::debug!("[Path::walk] {:?}", split_path(path));
        for p in split_path(path) {
            match p {
                ".." => {
                    dentry = dentry.parent().ok_or(SysError::ENOENT)?;
                }
                name => match dentry.lookup(name) {
                    Ok(sub_dentry) => {
                        log::debug!("[Path::walk] sub dentry {}", sub_dentry.name());
                        dentry = sub_dentry
                    }
                    Err(e) => {
                        log::error!("[Path::walk] {e:?} when walking in path {path}");
                        return Err(e);
                    }
                },
            }
        }
        Ok(dentry)
    }
}
```

下面给出`Path`的结构和描述：

```rust
pub struct Path {
    /// The root of the file system
    root: Arc<dyn Dentry>,
    /// The directory to start searching from
    start: Arc<dyn Dentry>,
    /// The path to search for
    path: String,
}
```

## 辅助数据结构及其操作

### FdTable

当一个进程调用 `open()` 系统调用，内核会创建一个文件对象来维护被进程打开的文件的信息，但是内核并不会将这个文件对象返回给进程，而是将一个非负整数返回，即 `open()` 系统调用的返回值是一个非负整数，这个整数称作文件描述符。文件描述符和文件对象一一对应，而维护二者对应关系的数据结构，就是文件描述符表。在实现细节中，文件描述符表本质是一个数组，数组中每一个元素就是文件对象，而元素下标就是文件对象对应的文件描述符。

文件描述符表的操作由 `FdTable` 描述，其形式为：

```rust
impl FdTable {
    /// Create a new file descriptor table and create three file descriptors for
    /// 1. stdin
    /// 2. stdout
    /// 3. stderr
    pub fn new() -> Self {
        let mut vec: Vec<Option<Arc<dyn File>>> = Vec::new();
        vec.push(Some(TTY.get().unwrap().clone()));
        vec.push(Some(TTY.get().unwrap().clone()));
        vec.push(Some(TTY.get().unwrap().clone()));
        Self { table: vec }
    }

    /// Find the minimium released fd
    fn find_free_slot(&self) -> Option<usize> {
        (0..self.table.len()).find(|fd| self.table[*fd].is_none())
    }

    /// Find fd that is no less than lower_bound
    fn find_free_slot_and_create(&mut self, lower_bound: usize) -> usize {
        if lower_bound > self.table.len() {
            for _ in self.table.len()..lower_bound {
                self.table.push(None)
            }
            lower_bound
        } else {
            for idx in lower_bound..self.table.len() {
                if self.table[idx].is_none() {
                    return idx;
                }
            }
            self.table.push(None);
            self.table.len()
        }
    }

    /// Find the minimium released fd, will alloc a fd if necessary, and insert
    /// the `file` into the table.
    pub fn alloc(&mut self, file: Arc<dyn File>) -> SysResult<Fd> {
        if let Some(fd) = self.find_free_slot() {
            self.table[fd] = Some(file);
            Ok(fd)
        } else {
            self.table.push(Some(file));
            Ok(self.table.len() - 1)
        }
    }

    /// Get file according to file descriptor
    pub fn get(&self, fd: Fd) -> SysResult<Arc<dyn File>> {
        if fd >= self.table.len() {
            Err(SysError::EBADF)
        } else {
            let file = self.table[fd].clone().ok_or(SysError::EBADF)?;
            Ok(file)
        }
    }

    /// Remove file from fd table according to fd
    pub fn remove(&mut self, fd: Fd) -> SysResult<()> {
        if fd >= self.table.len() {
            Err(SysError::EBADF)
        } else {
            self.table[fd] = None;
            Ok(())
        }
    }

    /// Insert file into fd table at the position of fd
    pub fn insert(&mut self, fd: Fd, file: Arc<dyn File>) -> SysResult<()> {
        if fd >= self.table.len() {
            for _ in self.table.len()..fd {
                self.table.push(None)
            }
            self.table.push(Some(file));
            Ok(())
        } else {
            self.table[fd] = Some(file);
            Ok(())
        }
    }

    /// Called by the dup(2) system call. Allocates a new file descriptor that refers 
    /// to the same open file description as the descriptor old_fd.
    pub fn dup(&mut self, old_fd: Fd) -> SysResult<Fd> {
        let file = self.get(old_fd)?;
        self.alloc(file)
    }

    /// Called by the dup2(2) system call. Allocates a new file descriptor new_fd
    /// that refers to the same open file description as the descriptor old_fd.
    pub fn dup3(&mut self, old_fd: Fd, new_fd: Fd) -> SysResult<Fd> {
        let file = self.get(old_fd)?;
        self.insert(new_fd, file)?;
        Ok(new_fd)
    }

    /// Allocates a new file descriptor that refers 
    /// to the same open file description as the descriptor old_fd.
    /// new file descriptor is no less than lower_bound
    pub fn dup_with_bound(&mut self, old_fd: Fd, lower_bound: usize) -> SysResult<Fd> {
        let file = self.get(old_fd)?;
        let new_fd = self.find_free_slot_and_create(lower_bound);
        self.insert(new_fd, file);
        Ok(new_fd)
    }

    /// Called by execve(2) system call. When a new program is executed by current process,
    /// check all the files that were opened by the current process. If the file contains 
    /// close_on_exec flag, remove it from the fd table and disable its file descriptor. 
    /// Otherwise, keep the file descriptor valid and the new process can still access to
    /// the file with the file descriptor.
    pub fn close_on_exec(&mut self) {
        for (_, slot) in self.table.iter_mut().enumerate() {
            if let Some(file) = slot {
                if file.flags().contains(OpenFlags::O_CLOEXEC) {
                    *slot = None;
                }
            }
        }
    }

    /// Take the ownership of the given fd.
    pub fn take(&mut self, fd: Fd) -> Option<Arc<dyn File>> {
        if fd >= self.table.len() {
            None
        } else {
            self.table[fd].take()
        }
    }

    /// Get the length of file descriptor table
    pub fn len(&self) -> usize {
        self.table.len()
    }
}
```

文件描述符表对象由 `FdTable` 结构体描述：

```rust
pub struct FdTable {
    /// File descriptor table is actually a Vector
    table: Vec<Option<Arc<dyn File>>>,
}
```

### Pipe

管道Pipe是一种基本的进程间通信机制。它允许一个进程将数据流输出到另一个进程。文件系统来实现管道通信，实现方式就是创建一个FIFO类型的管道文件，文件内容就是一个缓冲区，同时创建两个文件对象和对应的两个文件描述符。两个文件对象都指向这个管道文件，一个文件负责向管道的缓冲区中写入内容，一个负责从管道的缓冲区中读出内容。

管道文件的数据结构由 `PipeInode` 描述：

```rust
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
}
```

`PipeInode` 是对VFS中 `Inode` 数据结构的一个实现，包含元数据、缓冲区和管道是否关闭的信息。`PipeInode` 的关闭则采用了Rust语言原生支持的RAII原则，在 `Drop` 中实现管道的关闭。

```rust
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
```

对管道进行读写的两个文件对象，`PipeReadFile` 和 `PipeWriteFile` ，则是对VFS中 `File` 的实现：

```rust
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
```

`PipeReadFile` 负责从管道文件中读出数据，因此在实现 `File` 的时候，只实现 `read` 方法：

```rust
impl File for PipeReadFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn read(&self, offset: usize, buf: &mut [u8]) -> systype::SysResult<usize> {
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
}
```

调用`read` 方法，当缓冲区没有数据时，读进程会主动让出CPU资源，等待异步调度器调度到本进程时再次查看缓冲区是否有数据。上述步骤会一直重复，知道写进程将数据写入缓冲区，之后读进程将数据从缓冲区读出。

`PipeWriteFile` 负责向管道文件写入数据，在实现 `File` 的时候，只实现 `write` 方法：

```rust
impl File for PipeWriteFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn write(&self, offset: usize, buf: &[u8]) -> systype::SysResult<usize> {
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
}
```

管道文件在写进程的 `PipeWriteFile` 生命周期结束时关闭，因此在 `PipeWriteFile` 的 `Drop` 中关闭管道文件：

```rust
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
```

## 已实现的相关系统调用

- `getcwd`
- `pipe`
- `dup`
- `dup2`
- `chdir`
- `open`
- `close`
- `getdents64`
- `read`
- `write`
- `linkat`
- `unlinkat`
- `mkdirat`
- `umount2`
- `mount`
- `fstat`
- `fstatat`
