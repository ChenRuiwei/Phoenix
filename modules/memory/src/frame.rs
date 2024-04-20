//! Implementation of [`FrameAllocator`] which
//! controls all the frames in the operating system.
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};

use bitmap_allocator::BitAlloc;
use spin::Once;
use sync::mutex::SpinNoIrqLock;

use crate::{PhysAddr, PhysPageNum};

/// Manage a frame which has the same lifecycle as the tracker.
pub struct FrameTracker {
    /// PPN of the frame
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    /// Create an empty `FrameTracker`
    pub fn new(ppn: PhysPageNum) -> Self {
        // page cleaning
        ppn.empty_the_page();
        Self { ppn }
    }
}

impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker:PPN={:#x}", self.ppn.0))
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        dealloc_frame(self.ppn);
    }
}

pub type FrameAllocator = bitmap_allocator::BitAlloc16M;

pub static FRAME_ALLOCATOR: SpinNoIrqLock<FrameAllocator> =
    SpinNoIrqLock::new(FrameAllocator::DEFAULT);

static START_PPN: Once<PhysPageNum> = Once::new();
static END_PPN: Once<PhysPageNum> = Once::new();

/// Initiate the frame allocator, using `VPNRange`
pub fn init_frame_allocator(start: PhysPageNum, end: PhysPageNum) {
    START_PPN.call_once(|| start);
    END_PPN.call_once(|| end);
    FRAME_ALLOCATOR
        .lock()
        .insert(0..(END_PPN.get().unwrap().0 - START_PPN.get().unwrap().0));
    log::info!(
        "frame allocator init finshed, start {:#x}, end {:#x}",
        PhysAddr::from(start),
        PhysAddr::from(end)
    );
}

/// Allocate a frame
pub fn alloc_frame() -> FrameTracker {
    FRAME_ALLOCATOR
        .lock()
        .alloc()
        .map(|u| FrameTracker::new((u + START_PPN.get().unwrap().0).into()))
        .expect("frame space not enough")
}

/// Allocate contiguous frames
pub fn alloc_frames(size: usize, align_log2: usize) -> Vec<FrameTracker> {
    let first_frame = FRAME_ALLOCATOR
        .lock()
        .alloc_contiguous(size, align_log2)
        .unwrap();

    (first_frame..first_frame + size)
        .map(|u| FrameTracker::new((u + START_PPN.get().unwrap().0).into()))
        .collect()
}

/// Deallocate a frame
pub fn dealloc_frame(ppn: PhysPageNum) {
    FRAME_ALLOCATOR
        .lock()
        .dealloc(ppn.0 - START_PPN.get().unwrap().0);
}
