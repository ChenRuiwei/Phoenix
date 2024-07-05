pub mod virtio_blk;
pub mod virtio_net;

use alloc::vec::Vec;
use core::{marker::PhantomData, ptr::NonNull};

use device_core::{error::DevResult, BaseDeviceOps, DeviceType};
use memory::{
    address::vaddr_to_paddr, alloc_frames, dealloc_frame, FrameTracker, PhysAddr, PhysPageNum,
    VirtAddr,
};
use virtio_drivers::{transport::mmio::MmioTransport, BufferDirection};

use crate::manager::{DeviceEnum, DriverProbe};

pub struct VirtioHalImpl;

unsafe impl virtio_drivers::Hal for VirtioHalImpl {
    fn dma_alloc(
        pages: usize,
        _direction: BufferDirection,
    ) -> (virtio_drivers::PhysAddr, NonNull<u8>) {
        let pa = alloc_frames(pages);
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
        // PERF:参考arceos或许可以一次性删除多个页面？
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

/// A trait for VirtIO device meta information.
pub trait VirtIoDevMeta {
    const DEVICE_TYPE: DeviceType;

    type Device: BaseDeviceOps;
    type Driver = VirtIoDriver<Self>;

    fn try_new(transport: MmioTransport) -> DevResult<DeviceEnum>;
}

/// A common driver for all VirtIO devices that implements [`DriverProbe`].
pub struct VirtIoDriver<D: VirtIoDevMeta + ?Sized>(PhantomData<D>);

// impl<D: VirtIoDevMeta> DriverProbe for VirtIoDriver<D> {
//     fn probe_mmio(mmio_base: usize, mmio_size: usize) -> Option<DeviceEnum> {
//         let base_vaddr = phys_to_virt(mmio_base.into());
//         if let Some((ty, transport)) =
//             driver_virtio::probe_mmio_device(base_vaddr.as_mut_ptr(),
// mmio_size)         {
//             if ty == D::DEVICE_TYPE {
//                 match D::try_new(transport) {
//                     Ok(dev) => return Some(dev),
//                     Err(e) => {
//                         warn!(
//                             "failed to initialize MMIO device at [PA:{:#x},
// PA:{:#x}): {:?}",                             mmio_base,
//                             mmio_base + mmio_size,
//                             e
//                         );
//                         return None;
//                     }
//                 }
//             }
//         }
//         None
//     }
// }
