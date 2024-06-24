pub mod virtio_blk;

use alloc::vec::Vec;
use core::ptr::NonNull;

use memory::{
    address::vaddr_to_paddr, alloc_frames, dealloc_frame, FrameTracker, PhysAddr, PhysPageNum,
    VirtAddr,
};
use virtio_drivers::BufferDirection;

pub struct VirtioHalImpl;

unsafe impl virtio_drivers::Hal for VirtioHalImpl {
    fn dma_alloc(
        pages: usize,
        _direction: BufferDirection,
    ) -> (virtio_drivers::PhysAddr, NonNull<u8>) {
        let mut pa = alloc_frames(pages);
        let ppn = pa.floor();
        for ppn in ppn..ppn + pages {
            ppn.clear_page();
        }
        (
            pa.0,
            NonNull::new(pa.to_offset().to_va().as_mut_ptr()).unwrap(),
        )
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

    unsafe fn mmio_phys_to_virt(paddr: virtio_drivers::PhysAddr, _size: usize) -> NonNull<u8> {
        NonNull::new(PhysAddr::from(paddr).to_offset().to_va().as_mut_ptr()).unwrap()
    }

    unsafe fn share(
        buffer: NonNull<[u8]>,
        _direction: BufferDirection,
    ) -> virtio_drivers::PhysAddr {
        memory::vaddr_to_paddr((buffer.as_ptr() as *const u8 as usize).into()).into()
    }

    unsafe fn unshare(
        _paddr: virtio_drivers::PhysAddr,
        _buffer: NonNull<[u8]>,
        _direction: BufferDirection,
    ) {
    }
}
