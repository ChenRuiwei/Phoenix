use memory::{frame_alloc, FrameTracker, PhysPageNum};

pub struct Page {
    pub frame: FrameTracker,
}

impl Page {
    pub fn new() -> Self {
        Self {
            frame: frame_alloc(),
        }
    }

    pub fn ppn(&self) -> PhysPageNum {
        self.frame.ppn
    }
}
