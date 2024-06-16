pub mod virtio_blk;

use alloc::vec::Vec;
use core::ptr::NonNull;

use memory::{alloc_frames, dealloc_frame, FrameTracker, PhysAddr, PhysPageNum, VirtAddr};
use sync::mutex::SpinNoIrqLock;
use virtio_drivers::BufferDirection;

static QUEUE_FRAMES: SpinNoIrqLock<Vec<FrameTracker>> = SpinNoIrqLock::new(Vec::new());

pub struct VirtioHalImpl;

unsafe impl virtio_drivers::Hal for VirtioHalImpl {
    fn dma_alloc(
        pages: usize,
        _direction: BufferDirection,
    ) -> (virtio_drivers::PhysAddr, NonNull<u8>) {
        let mut ppn_base = PhysPageNum(0);
        // We lock the queue in advance to ensure that we can get a contiguous area
        let mut queue_frames_locked = QUEUE_FRAMES.lock();
        // TODO: what does align_log2 mean
        let mut frames = alloc_frames(pages);
        for i in 0..pages {
            let frame = frames.pop().unwrap();
            if i == pages - 1 {
                ppn_base = frame.ppn;
            }
            queue_frames_locked.push(frame);
        }
        let pa: PhysAddr = ppn_base.into();
        (pa.0, unsafe {
            NonNull::new_unchecked(pa.to_offset().to_va().as_mut_ptr())
        })
    }

    unsafe fn dma_dealloc(
        paddr: virtio_drivers::PhysAddr,
        _vaddr: NonNull<u8>,
        pages: usize,
    ) -> i32 {
        let pa = PhysAddr::from(paddr);
        let ppn_base: PhysPageNum = pa.into();
        for ppn in ppn_base..ppn_base + pages {
            dealloc_frame(ppn);
        }
        0
    }

    unsafe fn mmio_phys_to_virt(
        paddr: virtio_drivers::PhysAddr,
        _size: usize,
    ) -> core::ptr::NonNull<u8> {
        log::debug!("phy2virt: addr {:#x}", paddr);
        NonNull::new_unchecked(PhysAddr::from(paddr).to_offset().to_va().as_mut_ptr())
    }

    unsafe fn share(
        buffer: core::ptr::NonNull<[u8]>,
        _direction: virtio_drivers::BufferDirection,
    ) -> virtio_drivers::PhysAddr {
        VirtAddr::from(buffer.as_ptr() as *const usize as usize)
            .to_offset()
            .to_pa()
            .bits()
    }

    unsafe fn unshare(
        _paddr: virtio_drivers::PhysAddr,
        _buffer: core::ptr::NonNull<[u8]>,
        _direction: virtio_drivers::BufferDirection,
    ) {
    }
}
