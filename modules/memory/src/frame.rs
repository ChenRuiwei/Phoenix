//! Implementation of [`FrameAllocator`] which
//! controls all the frames in the operating system.
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};

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

trait FrameAllocator {
    fn init(&mut self, start: PhysPageNum, end: PhysPageNum);
    fn alloc(&mut self) -> Option<PhysPageNum>;
    fn dealloc(&mut self, ppn: PhysPageNum);
    fn alloc_contig(&mut self, count: usize) -> Vec<PhysPageNum>;
}

/// an implementation for frame allocator
pub struct StackFrameAllocator {
    current: usize,
    end: usize,
    recycled: Vec<usize>,
}

impl StackFrameAllocator {
    const fn new() -> Self {
        Self {
            current: 0,
            end: 0,
            recycled: Vec::new(),
        }
    }
}

impl FrameAllocator for StackFrameAllocator {
    fn init(&mut self, start: PhysPageNum, end: PhysPageNum) {
        self.current = start.into();
        self.end = end.into();
    }

    fn alloc(&mut self) -> Option<PhysPageNum> {
        if let Some(ppn) = self.recycled.pop() {
            Some(ppn.into())
        } else if self.current == self.end {
            panic!("cannot alloc!!!!!!! current {:#x}", self.current)
        } else {
            self.current += 1;
            Some((self.current - 1).into())
        }
    }

    fn dealloc(&mut self, ppn: PhysPageNum) {
        // ppn.bytes_array().fill(0);
        let ppn = ppn.0;
        // validity check
        if ppn >= self.current || self.recycled.iter().any(|&v| v == ppn) {
            panic!("Frame ppn={:#x} has not been allocated!", ppn);
        }
        // recycle
        self.recycled.push(ppn);
    }

    fn alloc_contig(&mut self, count: usize) -> Vec<PhysPageNum> {
        let mut ret = Vec::with_capacity(count);
        for _ in 0..count {
            if self.current == self.end {
                panic!("cannot alloc!!!!!!! current {:#x}", self.current)
            } else {
                self.current += 1;
                ret.push((self.current - 1).into());
            }
        }
        ret
    }
}

type FrameAllocatorImpl = StackFrameAllocator;

pub static FRAME_ALLOCATOR: SpinNoIrqLock<FrameAllocatorImpl> =
    SpinNoIrqLock::new(FrameAllocatorImpl::new());

/// Initiate the frame allocator, using `VPNRange`
pub fn init_frame_allocator(start: PhysPageNum, end: PhysPageNum) {
    FRAME_ALLOCATOR.lock().init(start, end);
    log::info!(
        "frame allocator init finshed, start {:#x}, end {:#x}",
        usize::from(PhysAddr::from(start)),
        usize::from(PhysAddr::from(end))
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
        .map(|u| FrameTracker::new(u.into()))
        .expect("frame space not enough")
}

/// Allocate contiguous frames
pub fn frame_alloc_contig(count: usize) -> Vec<FrameTracker> {
    FRAME_ALLOCATOR
        .lock()
        .alloc_contig(count)
        .iter()
        .map(|p| FrameTracker::new(*p))
        .collect()
}

/// Deallocate a frame
pub fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.lock().dealloc(ppn);
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
