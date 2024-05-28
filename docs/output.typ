= 虚拟文件系统
<虚拟文件系统>
虚拟文件系统（Virtual File System，简称
VFS）是内核中负责与各种字符流（如磁盘文件，IO
设备等等）对接，并对外提供操作接口的子系统。它为用户程序提供了一个统一的文件和文件系统操作接口，屏蔽了不同文件系统之间的差异和操作细节。这意味着，用户程序可以使用标准的系统调用，如
`open()`、`read()`、`write()`
来操作文件，而无需关心文件实际存储在哪种类型的文件系统或存储介质上。

Phoenix OS 的虚拟文件系统以 Linux 为师，并充分结合 Rust
语言的特性，从面向对象的角度出发对虚拟文件系统进行了设计和优化。

== 虚拟文件系统结构
<虚拟文件系统结构>
目前虚拟文件系统包含 `SuperBlock`, `Inode`, `Dentry`,
`File`等核心数据结构，也包含 `FdTable`,
`Pipe`等用于实现系统调用的辅助数据结构。

== 核心数据结构
<核心数据结构>
=== SuperBlock
<superblock>
超级块对象用于存储特定文件系统的信息，通常对应于存放在磁盘特定扇区中的文件系统超级块。超级块是对文件系统的具象，换句话说，一个超级块对应一个文件系统的实例。对于基于磁盘上的文件系统，当文件系统被挂载内核时，内核需要读取文件系统位于磁盘上的超级块，并在内存中构造超级块对象；当文件系统卸载时，需要将超级块对象释放，并将内存中的被修改的数据写回到磁盘。对于并非基于磁盘上的文件系统（如基于内存的文件系统，比如
sysfs），就只需要在内存构造独立的超级块。

超级块由 `SuperBlock` trait 定义，如下：

```rust
pub trait SuperBlock: Send + Sync {
    /// Get metadata of this super block.
    fn meta(&self) -> &SuperBlockMeta;

    /// Get filesystem statistics.
    fn stat_fs(&self) -> SysResult<StatFs>;

    /// Called when VFS is writing out all dirty data associated with a
    /// superblock.
    fn sync_fs(&self, wait: isize) -> SysResult<()>;
}
```

与传统的面向对象编程语言（如 Java 或 C++）不同，Rust 鼓励使用组合和
trait
来实现代码复用和抽象，而不是使用继承。如果要实现继承特性，就需要设计
Meta
结构体来表示对基类的抽象，为了使用继承来简化设计，减少冗余代码，超级块基类对象的设计由
`SuperBlockMeta` 结构体表示。

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

对于具体的文件系统，只需要实现自己的超级块对象，其中包含
`SuperBlockMeta` 的字段，就能完成继承对超级块基类的继承。 比如对 FAT32
文件系统，我们只需要构造这样一个 `FatSuperBlock` 对象就能完成对 VFS
`SuperBlockMeta` 的继承，同时，只需要为 `FatSuperBlock` 实现
`SuperBlock` trait 就能实现对接口方法的多态行为。这样就能在 Rust
语言中使用面向对象的设计来大大简化具体文件系统对 VFS 的对接代码量。

```rust
pub struct FatSuperBlock {
    meta: SuperBlockMeta,
    fs: Arc<FatFs>,
}
```

=== Inode
<inode>
索引节点是对文件系统中文件信息的抽象。对于文件系统中的文件来说，文件名可以随时更改，但是索引节点对文件一定是唯一的，并且随文件的存在而存在。

索引节点由 `Inode` trait 表示，如下：

```rust
pub trait Inode: Send + Sync + DowncastSync {
    /// Get metadata of this Inode
    fn meta(&self) -> &InodeMeta;

    /// Get attributes of this file
    fn get_attr(&self) -> SysResult<Stat>;
}
```

索引节点对象由 `InodeMeta` 结构体表示，下面给出它的结构和描述：

```rust
pub struct InodeMeta {
    /// Inode number.
    pub ino: usize,
    /// Mode of inode.
    pub mode: InodeMode,
    /// Super block this inode belongs to
    pub super_block: Weak<dyn SuperBlock>,
    /// Protect mutable data with mutex.
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

=== Dentry
<dentry>
目录项是管理文件在目录树中的信息的结构体，是对文件路径的抽象。在文件系统中，以挂载点，即文件系统的根目录为根节点，按照文件夹与下属文件的父子关系逐级向下，形成一个目录树的结构。目录树的每个节点对应一个目录项，而每一个目录项都指向一个文件的索引节点。Dentry
存在的必要性源于 Unix
将文件本身与文件名解耦合的设计，这使得不同的目录项可以指向相同的索引节点（即硬链接）。虽然竞赛规定使用的
FAT32
的设计是将路径与文件本身耦合的，这也导致其不支持硬链接技术，而也正因此，往届很多作品并没有
Dentry 这个结构，而是将路径解析的功能保存在 Inode
结构体中，然而这并不符合 Unix 哲学，这种 VFS
设计并不能扩展到其他文件系统上。而我们认为遵守 Unix
设计哲学能有更好的扩展性，为了将来使 Phoenix
能够支持更多文件系统，我们选择遵守 Unix
设计规范，将路径与文件本身相分离，形成了 Dentry 和 Inode 这两者的抽象。

目录项与索引节点的多对一的映射关系使得文件系统只需要缓存目录项就能缓存对应的索引节点。而目录项的状态分为两种，一种是被使用的，即正常指向
Inode 的目录项，一种是负状态，即没有对应 Inode
的目录项。负目录项的存在是因为文件系统试图访问不存在的路径，或者文件被删除了。如果没有负目录项，文件系统会到磁盘上遍历目录结构体并检查这个文件的确不存在，这个失败的查找很浪费资源，为了尽量减少对磁盘的
IO 访问，Phoenix 的文件系统会缓存这些负目录项以便快速解析这些路径。

目录项的操作由 `Dentry` trait 描述，定义如下：

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
}
```

目录项对象由 `DentryMeta` 结构体表示：

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

=== File
<file>
文件对象是进程已打开的文件在内存中的表示。文件对象由系统调用 `open()`
创建，由系统调用 `close()`
撤销，所有文件相关的系统调用实际上都是文件对象定义的操作。文件对象与文件系统中的文件并不是一一对应的关系，因为多个进程可能会同时打开同一个文件，也就会创建多个文件对象，但这些文件对象指向的索引节点都是同一个索引节点，即同一个文件。

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
    fn ioctl(&self, cmd: usize, arg: usize) -> SyscallResult;

    /// called when a process wants to check if there is activity on this file and (optionally)
    /// go to sleep until there is activity.
    /// Called by the select(2) and poll(2) system calls
    async fn poll(&self, events: PollEvents) -> SysResult<PollEvents>;

    /// Called when the VFS needs to move the file position index.
    ///
    /// Return the result offset.
    fn seek(&self, pos: SeekFrom) -> SysResult<usize>;
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

=== FileSystemType
<filesystemtype>
`FileSystemType`
用来描述各种特定文件系统类型的功能和行为，并负责管理每种文件系统下的所有文件系统实例以及对应的超级块。

`FileSystemType` trait 的定义如下：

```rust
pub trait FileSystemType: Send + Sync {
    fn meta(&self) -> &FileSystemTypeMeta;

    /// Call when a new instance of this filesystem should be mounted.
    fn base_mount(
        self: Arc<Self>,
        name: &str,
        parent: Option<Arc<dyn Dentry>>,
        flags: MountFlags,
        dev: Option<Arc<dyn BlockDevice>>,
    ) -> SysResult<Arc<dyn Dentry>>;

    /// Call when an instance of this filesystem should be shut down.
    fn kill_sb(&self, sb: Arc<dyn SuperBlock>) -> SysResult<()>;
}
```

`FileSystemType`的设计由 `FileSystemTypeMeta`
结构体表示，下面给出它的结构和描述：

```rust
pub struct FileSystemTypeMeta {
    /// Name of this file system type.
    name: String,
    /// Super blocks.
    supers: Mutex<BTreeMap<String, Arc<dyn SuperBlock>>>,
}
```

=== Path
<path>
`Path` 结构体的主要用来实现路径解析，由于我们在 `DentryMeta` 中使用
`BTreeMap`
来对缓存一个文件夹下的所有子目录项，因此我们能够在内存中快速进行路径解析，而无需重复访问磁盘进行耗时的
IO 操作。

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

由于我们已经通过 Dentry 实现了对目录树的抽象，路径解析的实现非常简单。

```rust
impl Path {
    /// Walk until path has been resolved.
    pub fn walk(&self) -> SysResult<Arc<dyn Dentry>> {
        let path = self.path.as_str();
        let mut dentry = if is_absolute_path(path) {
            self.root.clone()
        } else {
            self.start.clone()
        };
        for p in split_path(path) {
            match p {
                ".." => {
                    dentry = dentry.parent().ok_or(SysError::ENOENT)?;
                }
                name => match dentry.lookup(name) {
                    Ok(sub_dentry) => {
                        dentry = sub_dentry
                    }
                    Err(e) => {
                        return Err(e);
                    }
                },
            }
        }
        Ok(dentry)
    }
}
```

== 其他数据结构
<其他数据结构>
=== FdTable
<fdtable>
Unix 设计哲学将文件本身抽象成
Inode，其保存了文件的元数据；将内核打开的文件抽象成
File，其保存了当前读写文件的偏移量以及文件打开的标志；进程只能看见文件描述符，文件描述符由进程结构体中的文件描述符表进行处理。

当一个进程调用 `open()`
系统调用，内核会创建一个文件对象来维护被进程打开的文件的信息，但是内核并不会将这个文件对象返回给进程，而是将一个非负整数返回，即
`open()`
系统调用的返回值是一个非负整数，这个整数称作文件描述符。文件描述符和文件对象一一对应，而维护二者对应关系的数据结构，就是文件描述符表。在实现细节中，文件描述符表本质是一个数组，数组中每一个元素就是文件对象，而元素下标就是文件对象对应的文件描述符。

=== Pipe
<pipe>
管道 Pipe
是一种基本的进程间通信机制。它允许一个进程将数据流输出到另一个进程。文件系统来实现管道通信，实现方式就是创建一个
FIFO
类型的管道文件，文件内容就是一个缓冲区，同时创建两个文件对象和对应的两个文件描述符。两个文件对象都指向这个管道文件，一个文件负责向管道的缓冲区中写入内容，一个负责从管道的缓冲区中读出内容。

管道文件的数据结构由 `PipeInode` 描述：

```rust
pub struct PipeInode {
    meta: InodeMeta,
    is_closed: Mutex<bool>,
    buf: Mutex<AllocRingBuffer<u8>>,
}
```

`PipeInode` 是对 VFS 中 `Inode`
数据结构的一个实现，包含元数据、缓冲区和管道是否关闭的信息。

对管道进行读写的两个文件对象，`PipeReadFile` 和 `PipeWriteFile` ，则是对
VFS 中 `File` 的实现：

```rust
pub struct PipeWriteFile {
    meta: FileMeta,
}

pub struct PipeReadFile {
    meta: FileMeta,
}
```

`PipeReadFile` 负责从管道文件中读出数据，因此在实现 `File`
的时候，只实现 `read` 方法：

```rust
impl File for PipeReadFile {
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

调用`read` 方法，当缓冲区没有数据时，读进程会主动让出 CPU
资源，等待异步调度器调度到本进程时再次查看缓冲区是否有数据。上述步骤会一直重复，知道写进程将数据写入缓冲区，之后读进程将数据从缓冲区读出。

`PipeWriteFile` 负责向管道文件写入数据，在实现 `File` 的时候，只实现
`write` 方法：

```rust
impl File for PipeWriteFile {
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

管道文件在写进程的 `PipeWriteFile` 生命周期结束时关闭，因此在
`PipeWriteFile` 的 `Drop` 中关闭管道文件：

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

== 已实现的相关系统调用
<已实现的相关系统调用>
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
