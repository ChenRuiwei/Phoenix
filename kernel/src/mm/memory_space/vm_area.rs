use alloc::{sync::Arc, vec::Vec};

use config::mm::PAGE_SIZE;
use log::{trace, warn};
use memory::{
    address::StepByOne,
    frame_alloc,
    page_table::{self, PTEFlags},
    MapPermission, PhysPageNum, VPNRange, VirtAddr, VirtPageNum,
};
use sync::cell::SyncUnsafeCell;
use systype::{GeneralRet, SyscallErr};

use crate::{
    mm::{page, Page, PageTable},
    processor::SumGuard,
    stack_trace, syscall,
};

/// Vm area type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VmAreaType {
    /// Segments from elf file, e.g. text, rodata, data, bss
    Elf,
    /// Stack
    Stack,
    /// Brk
    Brk,
    /// Mmap
    Mmap,
    /// Shared memory
    Shm,
    /// Physical frames (for kernel mapping with an offset)
    Physical,
    /// MMIO (for kernel)
    Mmio,
}

pub struct VmArea {
    pub vpn_range: VPNRange,
    pub frames: Vec<Page>,
    pub map_perm: MapPermission,
    pub vma_type: VmAreaType,
}

impl Drop for VmArea {
    fn drop(&mut self) {
        stack_trace!();
        log::debug!(
            "[VmArea::drop] drop vma, [{:#x}, {:#x}]",
            self.start_vpn().0,
            self.end_vpn().0
        );
    }
}

impl VmArea {
    /// Construct a new vma
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_perm: MapPermission,
        vma_type: VmAreaType,
    ) -> Self {
        stack_trace!();
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            frames: Vec::new(),
            vma_type,
            map_perm,
        }
    }

    pub fn start_vpn(&self) -> VirtPageNum {
        self.vpn_range.start()
    }

    pub fn end_vpn(&self) -> VirtPageNum {
        self.vpn_range.end()
    }

    pub fn map(&self, page_table: &mut PageTable) {
        if self.vma_type == VmAreaType::Physical {
            for vpn in self.vpn_range {
                page_table.map(
                    vpn,
                    VirtAddr::from(vpn).to_pa().into(),
                    self.map_perm.into(),
                )
            }
        }
    }
}
