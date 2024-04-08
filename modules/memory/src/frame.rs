//! Implementation of [`FrameAllocator`] which
//! controls all the frames in the operating system.
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};

use spin::Once;
use sync::mutex::SpinNoIrqLock;

use crate::{PhysAddr, PhysPageNum};

/// Manage a frame which has the same lifecycle as the tracker
pub struct FrameTracker {
    /// PPN of the frame
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    /// Create an empty `FrameTracker`
    pub fn new(ppn: PhysPageNum) -> Self {
        // page cleaning
        ppn.usize_array().fill(0);
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
        frame_dealloc(self.ppn);
    }
}

use bitmap_allocator::BitAlloc;

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
    debug_assert!({
        frame_allocator_test();
        true
    });
}

/// Allocate a frame
pub fn frame_alloc() -> FrameTracker {
    FRAME_ALLOCATOR
        .lock()
        .alloc()
        .map(|u| FrameTracker::new((u + START_PPN.get().unwrap().0).into()))
        .expect("frame space not enough")
}

/// Allocate contiguous frames
// Error: this function is wrong, alloc_contiguous returns only one usize which
// is not what we want
pub fn frame_alloc_contig(size: usize, align_log2: usize) -> Option<FrameTracker> {
    FRAME_ALLOCATOR
        .lock()
        .alloc_contiguous(size, align_log2)
        .map(|u| FrameTracker::new((u + START_PPN.get().unwrap().0).into()))
}

/// Deallocate a frame
pub fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR
        .lock()
        .dealloc(ppn.0 - START_PPN.get().unwrap().0);
}

/// a simple test for frame allocator
#[allow(unused)]
pub fn frame_allocator_test() {
    log::info!("frame_allocator_test start...");
    let mut v: Vec<FrameTracker> = Vec::new();
    for i in 0..5 {
        let frame = frame_alloc();
        log::info!("{:?}", frame);
        v.push(frame);
    }
    v.clear();
    for i in 0..5 {
        let frame = frame_alloc();
        log::info!("{:?}", frame);
        v.push(frame);
    }
    drop(v);
    log::info!("frame_allocator_test passed!");
}
