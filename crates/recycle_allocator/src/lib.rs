#![no_std]
#![no_main]
#![feature(const_binary_heap_constructor)]

extern crate alloc;
use alloc::vec::Vec;
/// Used for allocating pid & tid
// TODO: add maximium resource limit
pub struct RecycleAllocator {
    /// Current max id allocated
    current: usize,
    /// Hold deallocated id, will be recycled first when alloc happen
    recycled: Vec<usize>,
}

impl RecycleAllocator {
    /// Create an empty `RecycleAllocator`
    pub const fn new(init_val: usize) -> Self {
        RecycleAllocator {
            current: init_val,
            recycled: Vec::new(),
        }
    }

    /// Allocate an id
    pub fn alloc(&mut self) -> usize {
        if let Some(id) = self.recycled.pop() {
            id
        } else {
            self.current += 1;
            self.current - 1
        }
    }

    /// Recycle an id
    pub fn dealloc(&mut self, id: usize) {
        debug_assert!(id < self.current);
        debug_assert!(
            !self.recycled.iter().any(|iid| *iid == id),
            "id {} has been deallocated!",
            id
        );
        self.recycled.push(id);
    }
}
