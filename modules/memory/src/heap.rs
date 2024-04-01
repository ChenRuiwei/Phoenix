//! The global allocator
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};

use buddy_system_allocator::Heap;
use config::mm::KERNEL_HEAP_SIZE;
use sync::mutex::SpinNoIrqLock;

/// heap allocator instance
#[global_allocator]
static HEAP_ALLOCATOR: GlobalHeap = GlobalHeap::empty();

/// heap space
static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

/// panic when heap allocation error occurs
#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

struct GlobalHeap(SpinNoIrqLock<Heap<32>>);

impl GlobalHeap {
    const fn empty() -> Self {
        Self(SpinNoIrqLock::new(Heap::empty()))
    }
}

unsafe impl GlobalAlloc for GlobalHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0
            .lock()
            .alloc(layout)
            .ok()
            .map_or(core::ptr::null_mut::<u8>(), |allocation| {
                allocation.as_ptr()
            })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0.lock().dealloc(NonNull::new_unchecked(ptr), layout)
    }
}

/// initiate heap allocator
pub fn init_heap() {
    unsafe {
        let start = HEAP_SPACE.as_ptr() as usize;
        HEAP_ALLOCATOR.0.lock().init(start, KERNEL_HEAP_SIZE);
        log::info!(
            "[kernel] heap start {:#x}, end {:#x}",
            start,
            start + KERNEL_HEAP_SIZE
        );
    }
    heap_test()
}

/// heap test
#[allow(unused)]
pub fn heap_test() {
    use alloc::{boxed::Box, vec::Vec};
    extern "C" {
        fn _sbss();
        fn _ebss();
    }
    let bss_range = _sbss as usize.._ebss as usize;
    let a = Box::new(5);
    assert_eq!(*a, 5);
    assert!(bss_range.contains(&(a.as_ref() as *const _ as usize)));
    drop(a);
    let mut v: Vec<usize> = Vec::new();
    let max_len = (KERNEL_HEAP_SIZE - 10000) / core::mem::size_of::<usize>();
    for i in 0..500 {
        v.push(i);
    }
    for (i, val) in v.iter().take(500).enumerate() {
        assert_eq!(*val, i);
    }
    assert!(bss_range.contains(&(v.as_ptr() as usize)));
    drop(v);
    log::info!("heap_test passed!");
}
