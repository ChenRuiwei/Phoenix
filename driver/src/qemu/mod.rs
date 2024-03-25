use alloc::vec::Vec;
use core::ptr::NonNull;

use log::debug;
use memory::{
    frame_alloc_contig, frame_dealloc, FrameTracker, KernelAddr, PhysAddr, PhysPageNum, StepByOne,
    VirtAddr,
};
use sync::mutex::SpinNoIrqLock;
use virtio_drivers::{BufferDirection, Hal};

use crate::KERNEL_PAGE_TABLE;

pub mod virtio_blk;

static QUEUE_FRAMES: SpinNoIrqLock<Vec<FrameTracker>> = SpinNoIrqLock::new(Vec::new());
pub struct VirtioHal;

unsafe impl Hal for VirtioHal {
    fn dma_alloc(
        pages: usize,
        _direction: BufferDirection,
    ) -> (virtio_drivers::PhysAddr, NonNull<u8>) {
        let mut ppn_base = PhysPageNum(0);
        // We lock the queue in advance to ensure that we can get a contiguous area
        let mut queue_frames_locked = QUEUE_FRAMES.lock();
        let mut frames = frame_alloc_contig(pages);
        for i in 0..pages {
            let frame = frames.pop().unwrap();
            if i == pages - 1 {
                ppn_base = frame.ppn;
            }
            // println!("ppn {}", frame.ppn.0);
            // assert_eq!(frame.ppn.0, ppn_base.0 + i);
            queue_frames_locked.push(frame);
        }
        let pa: PhysAddr = ppn_base.into();
        (pa.0, unsafe {
            NonNull::new_unchecked(KernelAddr::from(pa).0 as *mut u8)
        })
    }

    unsafe fn dma_dealloc(
        paddr: virtio_drivers::PhysAddr,
        _vaddr: NonNull<u8>,
        pages: usize,
    ) -> i32 {
        let pa = PhysAddr::from(paddr);
        let mut ppn_base: PhysPageNum = pa.into();
        for _ in 0..pages {
            frame_dealloc(ppn_base);
            ppn_base.step();
        }
        0
    }

    unsafe fn mmio_phys_to_virt(
        paddr: virtio_drivers::PhysAddr,
        _size: usize,
    ) -> core::ptr::NonNull<u8> {
        debug!("phy2virt: addr {:#x}", paddr);
        NonNull::new_unchecked(KernelAddr::from(PhysAddr::from(paddr)).0 as *mut u8)
    }

    unsafe fn share(
        buffer: core::ptr::NonNull<[u8]>,
        _direction: virtio_drivers::BufferDirection,
    ) -> virtio_drivers::PhysAddr {
        unsafe {
            (*(KERNEL_PAGE_TABLE.as_ref().unwrap()).get())
                .translate_va(VirtAddr::from(buffer.as_ptr() as *const usize as usize))
                .unwrap()
                .0
        }
        // todo!()
    }

    unsafe fn unshare(
        _paddr: virtio_drivers::PhysAddr,
        _buffer: core::ptr::NonNull<[u8]>,
        _direction: virtio_drivers::BufferDirection,
    ) {
        // todo!()
    }
}

pub mod uart;

pub enum IntrSource {
    UART0 = 10,
    VIRTIO0 = 1,
    UnknownIntr,
}

impl From<usize> for IntrSource {
    fn from(value: usize) -> Self {
        match value {
            10 => Self::UART0,
            1 => Self::VIRTIO0,
            _ => Self::UnknownIntr,
        }
    }
}