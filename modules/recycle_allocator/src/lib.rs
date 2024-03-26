#![no_std]
#![no_main]
#![feature(const_binary_heap_constructor)]

extern crate alloc;
use alloc::collections::BinaryHeap;
use core::cmp::Reverse;

/// Used for allocating pid & tid
pub struct RecycleAllocator {
    /// Current max id allocated
    current: usize,
    /// Hold deallocated id, will be recycled first when alloc happen
    recycled: BinaryHeap<Reverse<usize>>,
}

impl RecycleAllocator {
    /// Create an empty `RecycleAllocator`
    pub const fn new(init_val: usize) -> Self {
        RecycleAllocator {
            current: init_val,
            recycled: BinaryHeap::new(),
        }
    }
    /// Allocate an id
    pub fn alloc(&mut self) -> usize {
        if let Some(Reverse(id)) = self.recycled.pop() {
            id
        } else {
            self.current += 1;
            self.current - 1
        }
    }
    /// Recycle an id
    pub fn dealloc(&mut self, id: usize) {
        assert!(id < self.current);
        assert!(
            !self.recycled.iter().any(|iid| (*iid).0 == id),
            "id {} has been deallocated!",
            id
        );
        self.recycled.push(Reverse(id));
    }
}
