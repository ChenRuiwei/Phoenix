use alloc::sync::Weak;

use config::{
    board::BLOCK_SIZE,
    mm::{PAGE_SIZE, PAGE_SIZE_BITS},
};
use log::{info, trace};
use memory::{frame_alloc, FrameTracker, MapPermission};
use sync::mutex::{SleepLock, SpinLock};
use systype::{GeneralRet, SyscallErr};

pub struct Page {
    frame: FrameTracker,
}

impl Page {
    pub fn new() -> Self {
        Self {
            frame: frame_alloc(),
        }
    }
}
