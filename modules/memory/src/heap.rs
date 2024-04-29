//! The global allocator
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};

// use buddy_system_allocator::Heap;
use config::mm::KERNEL_HEAP_SIZE;
use linked_list_allocator::Heap;
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

// struct GlobalHeap(SpinNoIrqLock<Heap<32>>);
//
// impl GlobalHeap {
//     const fn empty() -> Self {
//         Self(SpinNoIrqLock::new(Heap::empty()))
//     }
// }
//
// unsafe impl GlobalAlloc for GlobalHeap {
//     unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
//         self.0
//             .lock()
//             .alloc(layout)
//             .ok()
//             .map_or(core::ptr::null_mut::<u8>(), |allocation| {
//                 allocation.as_ptr()
//             })
//     }
//
//     unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
//         self.0.lock().dealloc(NonNull::new_unchecked(ptr), layout)
//     }
// }

struct GlobalHeap(SpinNoIrqLock<Heap>);

impl GlobalHeap {
    const fn empty() -> Self {
        Self(SpinNoIrqLock::new(Heap::empty()))
    }
}

unsafe impl GlobalAlloc for GlobalHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0
            .lock()
            .allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut::<u8>(), |allocation| {
                allocation.as_ptr()
            })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0
            .lock()
            .deallocate(NonNull::new_unchecked(ptr), layout)
    }
}

/// initiate heap allocator
pub fn init_heap_allocator() {
    unsafe {
        let start = HEAP_SPACE.as_ptr() as usize;
        // HEAP_ALLOCATOR.0.lock().init(start, KERNEL_HEAP_SIZE);
        HEAP_ALLOCATOR
            .0
            .lock()
            .init(HEAP_SPACE.as_mut_ptr(), KERNEL_HEAP_SIZE);
        log::info!(
            "[kernel] heap start {:#x}, end {:#x}",
            start,
            start + KERNEL_HEAP_SIZE
        );
    }
}
