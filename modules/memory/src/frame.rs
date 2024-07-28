//! Implementation of [`FrameAllocator`] which
//! controls all the frames in the operating system.

use alloc::vec::Vec;
use core::{
    cell::SyncUnsafeCell,
    fmt::{self, Debug, Formatter},
    ops::Range,
};

use bitmap_allocator::BitAlloc;
use sync::mutex::SpinNoIrqLock;

use crate::{PhysAddr, PhysPageNum};

/// Manage a frame which has the same lifecycle as the tracker.
pub struct FrameTracker {
    /// PPN of the frame.
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    /// Create an `FrameTracker`.
    ///
    /// It is the caller's duty to clean the frame.
    pub fn new(ppn: PhysPageNum) -> Self {
        Self { ppn }
    }

    /// Fill the page with zero.
    pub fn fill_zero(&self) {
        self.ppn.clear_page();
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

struct FrameAllocator {
    range_ppn: SyncUnsafeCell<Range<PhysPageNum>>,
    allocator: SpinNoIrqLock<bitmap_allocator::BitAlloc16M>,
}

impl FrameAllocator {
    fn init(&self, range_ppn: Range<PhysPageNum>) {
        unsafe { *self.range_ppn.get() = range_ppn };
    }

    fn range_ppn(&self) -> Range<PhysPageNum> {
        unsafe { &*self.range_ppn.get() }.clone()
    }
}

static FRAME_ALLOCATOR: FrameAllocator = FrameAllocator {
    range_ppn: SyncUnsafeCell::new(PhysPageNum::ZERO..PhysPageNum::ZERO),
    allocator: SpinNoIrqLock::new(bitmap_allocator::BitAlloc16M::DEFAULT),
};

/// Initiate the frame allocator, using `VPNRange`
pub fn init_frame_allocator(start: PhysPageNum, end: PhysPageNum) {
    FRAME_ALLOCATOR
        .allocator
        .lock()
        .insert(0..(end.0 - start.0));
    FRAME_ALLOCATOR.init(start..end);

    log::info!(
        "frame allocator init finshed, start {:#x}, end {:#x}",
        PhysAddr::from(start),
        PhysAddr::from(end)
    );
}

/// Allocate a frame
pub fn alloc_frame_tracker() -> FrameTracker {
    FRAME_ALLOCATOR
        .allocator
        .lock()
        .alloc()
        .map(|u| FrameTracker::new(FRAME_ALLOCATOR.range_ppn().start + u))
        .expect("frame space not enough")
}

/// Allocate contiguous frames
/// TODO: if this function is hot used, we should change the return type. Return
/// a vector is not efficient
pub fn alloc_frame_trackers(size: usize) -> Vec<FrameTracker> {
    let first_frame = FRAME_ALLOCATOR
        .allocator
        .lock()
        .alloc_contiguous(size, 0)
        .unwrap();

    (first_frame..first_frame + size)
        .map(|u| FrameTracker::new(FRAME_ALLOCATOR.range_ppn().start + u))
        .collect()
}

/// Allocate contiguous frames
pub fn alloc_frames(size: usize) -> PhysAddr {
    let ppn = FRAME_ALLOCATOR.range_ppn().start
        + FRAME_ALLOCATOR
            .allocator
            .lock()
            .alloc_contiguous(size, 0)
            .unwrap();
    ppn.to_paddr()
}

/// Deallocate a frame
pub fn dealloc_frame(ppn: PhysPageNum) {
    FRAME_ALLOCATOR
        .allocator
        .lock()
        .dealloc(ppn - FRAME_ALLOCATOR.range_ppn().start);
}
