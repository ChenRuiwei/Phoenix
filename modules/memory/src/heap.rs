//! The global allocator
use core::{
    self,
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};

use buddy_system_allocator::Heap as BuddyHeap;
use config::mm::KERNEL_HEAP_SIZE;
#[cfg(all(feature = "linked", not(feature = "buddy")))]
use linked_list_allocator::Heap as LinkedHeap;
use sbi_print::sbi_println;
use sync::mutex::SpinNoIrqLock;

#[cfg(all(feature = "buddy", not(feature = "linked")))]
type GlobalHeap = LockedBuddyHeap;
#[cfg(all(feature = "linked", not(feature = "buddy")))]
type GlobalHeap = LockedLinkedHeap;

/// heap allocator instance
#[global_allocator]
static HEAP_ALLOCATOR: GlobalHeap = GlobalHeap::empty();

/// heap space
#[link_section = ".bss.heap"]
static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

/// Panic when heap allocation error occurs.
#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    log::error!("heap alloc error");

    let inner = HEAP_ALLOCATOR.0.lock();
    let alloc_user = inner.stats_alloc_user();
    let alloc_actual = inner.stats_alloc_actual();
    let total_bytes = inner.stats_total_bytes();

    sbi_println!(
        "Heap allocation error, layout = {:?}, alloc_user: {}, alloc_actual: {}, total_bytes: {}",
        layout,
        alloc_user,
        alloc_actual,
        total_bytes
    );

    panic!();
}

struct LockedBuddyHeap(SpinNoIrqLock<BuddyHeap<32>>);

impl LockedBuddyHeap {
    const fn empty() -> Self {
        Self(SpinNoIrqLock::new(BuddyHeap::empty()))
    }

    unsafe fn init(&self, start: usize, size: usize) {
        self.0.lock().init(start, size)
    }
}

unsafe impl GlobalAlloc for LockedBuddyHeap {
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

#[cfg(all(feature = "linked", not(feature = "buddy")))]
struct LockedLinkedHeap(SpinNoIrqLock<LinkedHeap>);

#[cfg(all(feature = "linked", not(feature = "buddy")))]
impl LockedLinkedHeap {
    const fn empty() -> Self {
        Self(SpinNoIrqLock::new(LinkedHeap::empty()))
    }

    unsafe fn init(&self, start: usize, size: usize) {
        self.0.lock().init(start as *mut u8, size)
    }
}

#[cfg(all(feature = "linked", not(feature = "buddy")))]
unsafe impl GlobalAlloc for LockedLinkedHeap {
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

/// Initiate heap allocator.
pub fn init_heap_allocator() {
    unsafe {
        let start = HEAP_SPACE.as_ptr() as usize;
        HEAP_ALLOCATOR.init(start, KERNEL_HEAP_SIZE);
        log::info!(
            "[kernel] heap start {:#x}, end {:#x}",
            start,
            start + KERNEL_HEAP_SIZE
        );
    }
}
