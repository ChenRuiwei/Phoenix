use alloc::sync::Arc;
use core::{mem, ptr::NonNull};

use config::mm::VIRT_RAM_OFFSET;
use device_core::{error::DevError, Device, DeviceType};
use fdt::Fdt;
use log::{error, warn};
use memory::{alloc_frames, dealloc_frame, pte::PTEFlags, PhysAddr, PhysPageNum, VirtAddr};
use net::init_network;
use virtio_drivers::{
    transport::{
        self,
        mmio::{MmioTransport, VirtIOHeader},
        DeviceType as VirtIoDevType, Transport,
    },
    BufferDirection,
};

use crate::{blk::VirtIoBlkDev, kernel_page_table_mut, manager::DeviceManager, BLOCK_DEVICE};

pub(crate) const fn as_dev_err(e: virtio_drivers::Error) -> DevError {
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

pub(crate) const fn as_dev_type(t: transport::DeviceType) -> Option<DeviceType> {
    use transport::DeviceType::*;
    match t {
        Block => Some(DeviceType::Block),
        Network => Some(DeviceType::Net),
        GPU => Some(DeviceType::Display),
        _ => None,
    }
}

pub(crate) fn probe_devices_common<D, F>(
    dev_type: DeviceType,
    mmio_base: PhysAddr,
    mmio_size: usize,
    ret: F,
) -> Option<Arc<D>>
where
    D: Device + ?Sized,
    F: FnOnce(MmioTransport) -> Option<Arc<D>>,
{
    if let Some(transport) =
        probe_mmio_device(mmio_base.to_vaddr().as_mut_ptr(), mmio_size, Some(dev_type))
    {
        let dev = ret(transport)?;
        log::info!("created a new {:?} device: {:?}", dev.dtype(), dev.name());
        return Some(dev);
    }
    None
}

pub(crate) fn probe_mmio_device(
    reg_base: *mut u8,
    reg_size: usize,
    type_match: Option<DeviceType>,
) -> Option<MmioTransport> {
    use transport::mmio::VirtIOHeader;

    let header = NonNull::new(reg_base as *mut VirtIOHeader).unwrap();
    if let Ok(transport) = unsafe { MmioTransport::new(header) } {
        if type_match.is_none() || as_dev_type(transport.device_type()) == type_match {
            log::info!(
                "Detected virtio MMIO device with vendor id: {:#X}, device type: {:?}, version: {:?}",
                transport.vendor_id(),
                transport.device_type(),
                transport.version(),
            );
            Some(transport)
        } else {
            mem::forget(transport);
            None
        }
    } else {
        None
    }
}

impl DeviceManager {
    pub fn probe_virtio_device(&mut self, root: &Fdt) {
        let mut init_net: bool = false;
        let nodes = root.find_all_nodes("/soc/virtio_mmio");
        let mut reg;
        let mut base_paddr;
        let mut size;
        let mut irq_no;
        let mut base_vaddr;
        let mut header;

        for node in nodes {
            reg = node.reg().unwrap().next().unwrap();
            base_paddr = reg.starting_address as usize;
            size = reg.size.unwrap();
            irq_no = node.property("interrupts").unwrap().as_usize().unwrap();
            base_vaddr = base_paddr + VIRT_RAM_OFFSET;
            header = NonNull::new(base_vaddr as *mut VirtIOHeader).unwrap();

            // First map mmio memory since we need to read header.
            kernel_page_table_mut().ioremap(base_paddr, size, PTEFlags::R | PTEFlags::W);
            match unsafe { MmioTransport::new(header) } {
                Ok(transport) => match transport.device_type() {
                    VirtIoDevType::Block => {
                        if let Some(blk) = VirtIoBlkDev::try_new(base_paddr, size, None, transport)
                        {
                            BLOCK_DEVICE.call_once(|| blk.clone());
                            self.devices.insert(blk.dev_id(), blk);
                            continue;
                        }
                    }
                    // VirtIoDevType::Network => {
                    //     match NetDevice::try_new(transport) {
                    //         Ok(net) => {
                    //             init_network(net, false);
                    //             init_net = true;
                    //             continue;
                    //         }
                    //         Err(e) => error!(
                    //             "[virtio-net] failed to initialize MMIO device at [PA:{:#x},
                    // PA:{:#x}), {e:?}",
                    //             base_paddr,
                    //             base_paddr + size
                    //         ),
                    //     };
                    // }
                    _ => {
                        warn!(
                            "Unsupported VirtIO device type: {:?}",
                            transport.device_type()
                        );
                    }
                },
                Err(e) => {
                    log::info!(
                        "[init_virtio_device] Err {e:?} Can't initialize MmioTransport with {:?}",
                        reg
                    );
                }
            };
            kernel_page_table_mut().iounmap(base_vaddr, size);
        }

        // if !init_net {
        //     log::info!("[init_net] can't find qemu virtio-net. use
        // LoopbackDev to test");     init_network(LoopbackDev::new(),
        // true); }
    }
}

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
        (pa.0, NonNull::new(pa.to_vaddr().as_mut_ptr()).unwrap())
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
        NonNull::new(PhysAddr::from(paddr).to_vaddr().as_mut_ptr()).unwrap()
    }

    unsafe fn share(
        buffer: NonNull<[u8]>,
        _direction: BufferDirection,
    ) -> virtio_drivers::PhysAddr {
        VirtAddr::from(buffer.as_ptr() as *const u8 as usize)
            .to_paddr()
            .bits()
            .into()
    }

    unsafe fn unshare(
        _paddr: virtio_drivers::PhysAddr,
        _buffer: NonNull<[u8]>,
        _direction: BufferDirection,
    ) {
    }
}
