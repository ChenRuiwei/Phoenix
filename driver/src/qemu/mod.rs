pub mod virtio_blk;
pub mod virtio_net;

use alloc::{sync::Arc, vec::Vec};
use core::{marker::PhantomData, ptr::NonNull};

use config::mm::VIRT_RAM_OFFSET;
use device_core::{
    error::{DevError, DevResult},
    BaseDeviceOps, DeviceType,
};
use fdt::node::FdtNode;
use log::warn;
use memory::{
    address::vaddr_to_paddr, alloc_frames, dealloc_frame, pte::PTEFlags, FrameTracker, PhysAddr,
    PhysPageNum, VirtAddr,
};
use virtio_blk::VirtIOBlkDev;
use virtio_drivers::{
    transport::{
        self,
        mmio::{MmioTransport, VirtIOHeader},
        DeviceType as VirtIoDevType, Transport,
    },
    BufferDirection,
};
use virtio_net::VirtIoNet;

use crate::{kernel_page_table, manager::DeviceManager, BLOCK_DEVICE};

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

const fn as_dev_err(e: virtio_drivers::Error) -> DevError {
    use virtio_drivers::Error::*;
    match e {
        QueueFull => DevError::BadState,
        NotReady => DevError::Again,
        WrongToken => DevError::BadState,
        AlreadyUsed => DevError::AlreadyExists,
        InvalidParam => DevError::InvalidParam,
        DmaError => DevError::NoMemory,
        IoError => DevError::Io,
        Unsupported => DevError::Unsupported,
        ConfigSpaceTooSmall => DevError::BadState,
        ConfigSpaceMissing => DevError::BadState,
        _ => DevError::BadState,
    }
}

impl DeviceManager {
    pub fn init_virtio_device(&mut self, node: &FdtNode) {
        let reg = node.reg().unwrap().next().unwrap();
        let base_paddr = reg.starting_address as usize;
        let size = reg.size.unwrap();
        let base_vaddr = base_paddr + VIRT_RAM_OFFSET;
        let irq_no = node.property("interrupts").unwrap().as_usize().unwrap();
        let header = NonNull::new(base_vaddr as *mut VirtIOHeader).unwrap();
        kernel_page_table().map_kernel_region(
            base_vaddr.into()..(base_vaddr + size).into(),
            PTEFlags::R | PTEFlags::W,
        );
        match unsafe { MmioTransport::new(header) } {
            Ok(transport) => match transport.device_type() {
                VirtIoDevType::Block => {
                    if let Some(blk) = VirtIOBlkDev::try_new(base_paddr, size, irq_no, transport) {
                        BLOCK_DEVICE.call_once(|| blk.clone());
                        self.devices.insert(blk.dev_id(), blk);
                    }
                }
                VirtIoDevType::Network => {
                    if let Some(net) = VirtIoNet::try_new(base_paddr, size, irq_no, transport) {
                        self.devices.insert(net.dev_id(), net);
                    }
                }
                _ => {
                    warn!(
                        "Unsupported VirtIO device type: {:?}",
                        transport.device_type()
                    );
                }
            },
            Err(e) => {
                log::warn!(
                    "[init_virtio_device] Err {e:?} Can't initialize MmioTransport with {:?}",
                    reg
                );
            }
        };
    }
}
