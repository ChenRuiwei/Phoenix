//! Implementation of [`FrameAllocator`] which
//! controls all the frames in the operating system.

use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};

use bitmap_allocator::BitAlloc;
use sync::mutex::SpinNoIrqLock;

use crate::{PhysAddr, PhysPageNum};

/// Manage a frame which has the same lifecycle as the tracker.
pub struct FrameTracker {
    /// PPN of the frame.
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    /// Create an empty `FrameTracker`
    pub fn new(ppn: PhysPageNum) -> Self {
        // page cleaning
        // TODO: may be no need to always clean the page at first
        // ppn.empty_the_page();
        Self { ppn }
    }

    /// Fill the page with zero.
    pub fn clear(&self) {
        self.ppn.empty_the_page();
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

static mut START_PPN: Option<PhysPageNum> = None;
static mut END_PPN: Option<PhysPageNum> = None;

/// Initiate the frame allocator, using `VPNRange`
pub fn init_frame_allocator(start: PhysPageNum, end: PhysPageNum) {
    unsafe { START_PPN = Some(start) };
    unsafe { END_PPN = Some(end) };
    FRAME_ALLOCATOR.lock().insert(0..(end.0 - start.0));
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
        .map(|u| FrameTracker::new((u + unsafe { START_PPN.unwrap().0 }).into()))
        .expect("frame space not enough")
}

/// Allocate contiguous frames
/// TODO: if this function is hot used, we should change the return type. Return
/// a vector is not efficient
pub fn alloc_frames(size: usize) -> Vec<FrameTracker> {
    let first_frame = FRAME_ALLOCATOR.lock().alloc_contiguous(size, 0).unwrap();

    (first_frame..first_frame + size)
        .map(|u| FrameTracker::new((u + unsafe { START_PPN.unwrap().0 }).into()))
        .collect()
}

/// Deallocate a frame
pub fn dealloc_frame(ppn: PhysPageNum) {
    FRAME_ALLOCATOR
        .lock()
        .dealloc(ppn.0 - unsafe { START_PPN.unwrap().0 });
}
