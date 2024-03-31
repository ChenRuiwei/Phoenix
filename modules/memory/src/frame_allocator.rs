//! Implementation of [`FrameAllocator`] which
//! controls all the frames in the operating system.
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};

use sync::mutex::SpinNoIrqLock;

use crate::{VPNRange, VirtPageNum};

/// Manage a frame which has the same lifecycle as the tracker
pub struct FrameTracker {
    /// VPN of the frame
    pub vpn: VirtPageNum,
}

impl FrameTracker {
    /// Create an empty `FrameTracker`
    pub fn new(vpn: VirtPageNum) -> Self {
        // page cleaning
        vpn.usize_array().fill(0);
        Self { vpn }
    }
}

impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker:PPN={:#x}", self.vpn.0))
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        frame_dealloc(self.vpn);
    }
}

trait FrameAllocator {
    fn init(&mut self, vpn_range: VPNRange);
    fn alloc(&mut self) -> Option<VirtPageNum>;
    fn dealloc(&mut self, vpn: VirtPageNum);
    fn alloc_contig(&mut self, count: usize) -> Vec<VirtPageNum>;
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
    fn init(&mut self, vpn_range: VPNRange) {
        self.current = vpn_range.start().into();
        self.end = vpn_range.end().into()
    }

    fn alloc(&mut self) -> Option<VirtPageNum> {
        if let Some(vpn) = self.recycled.pop() {
            Some(vpn.into())
        } else if self.current == self.end {
            panic!("cannot alloc!!!!!!! current {:#x}", self.current)
        } else {
            self.current += 1;
            Some((self.current - 1).into())
        }
    }

    fn dealloc(&mut self, vpn: VirtPageNum) {
        // ppn.bytes_array().fill(0);
        let vpn = vpn.0;
        // validity check
        if vpn >= self.current || self.recycled.iter().any(|&v| v == vpn) {
            panic!("Frame ppn={:#x} has not been allocated!", vpn);
        }
        // recycle
        self.recycled.push(vpn);
    }

    fn alloc_contig(&mut self, count: usize) -> Vec<VirtPageNum> {
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
pub fn init_frame_allocator(vpn_range: VPNRange) {
    FRAME_ALLOCATOR.lock().init(vpn_range);
    log::info!(
        "frame allocator init finshed, start {:#x}, end {:#x}",
        usize::from(vpn_range.start()),
        usize::from(vpn_range.end())
    );
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
pub fn frame_dealloc(vpn: VirtPageNum) {
    FRAME_ALLOCATOR.lock().dealloc(vpn);
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
