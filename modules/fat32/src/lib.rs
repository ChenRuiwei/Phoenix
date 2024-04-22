#![no_std]
#![no_main]

use fatfs::{DefaultTimeProvider, Dir, File, FileSystem, LossyOemCpConverter};
use fatfs_shim::DiskCursor;
use sync::mutex::SpinNoIrqLock;

#[macro_use]
extern crate alloc;

pub mod fatfs_shim;
mod fs;

type Mutex<T> = SpinNoIrqLock<T>;

type FatDir = Dir<DiskCursor, DefaultTimeProvider, LossyOemCpConverter>;
type FatFile = File<DiskCursor, DefaultTimeProvider, LossyOemCpConverter>;
type FatFs = FileSystem<DiskCursor, DefaultTimeProvider, LossyOemCpConverter>;
