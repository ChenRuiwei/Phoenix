use alloc::vec::Vec;

use config::mm::PAGE_SIZE;
use memory::{pte::PTEFlags, StepByOne, VPNRange, VirtAddr, VirtPageNum};

use crate::{
    mm::{Page, PageTable},
    processor::env::SumGuard,
    stack_trace,
};

/// Vm area type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VmAreaType {
    /// Segments from user elf file, e.g. text, rodata, data, bss
    Elf,
    /// User Stack
    Stack,
    /// User Heap
    Heap,
    /// Mmap
    Mmap,
    /// Shared memory
    Shm,
    /// Physical frames (for kernel mapping with an offset)
    Physical,
    /// MMIO (for kernel)
    Mmio,
}

bitflags! {
    /// Map permission corresponding to that in pte: `R W X U`
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MapPermission: u16 {
        /// Readable
        const R = 1 << 1;
        /// Writable
        const W = 1 << 2;
        /// Excutable
        const X = 1 << 3;
        /// Accessible in U mode
        const U = 1 << 4;
        const RW = Self::R.bits() | Self::W.bits();
        const RX = Self::R.bits() | Self::X.bits();
        const WX = Self::W.bits() | Self::X.bits();
        const RWX = Self::R.bits() | Self::W.bits() | Self::X.bits();
        const URW = Self::U.bits() | Self::RW.bits();
        const URX = Self::U.bits() | Self::RX.bits();
        const UWX = Self::U.bits() | Self::WX.bits();
        const URWX = Self::U.bits() | Self::RWX.bits();
    }
}

impl From<MapPermission> for PTEFlags {
    fn from(perm: MapPermission) -> Self {
        let mut ret = Self::from_bits(0).unwrap();
        if perm.contains(MapPermission::U) {
            ret |= PTEFlags::U;
        }
        if perm.contains(MapPermission::R) {
            ret |= PTEFlags::R;
        }
        if perm.contains(MapPermission::W) {
            ret |= PTEFlags::W;
        }
        if perm.contains(MapPermission::X) {
            ret |= PTEFlags::X;
        }
        ret
    }
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

    pub fn map(&mut self, page_table: &mut PageTable) {
        if self.vma_type == VmAreaType::Physical {
            for vpn in self.vpn_range {
                page_table.map(
                    vpn,
                    VirtAddr::from(vpn).to_pa().into(),
                    self.map_perm.into(),
                )
            }
        } else {
            for vpn in self.vpn_range {
                let page = Page::new();
                page_table.map(vpn, page.ppn(), self.map_perm.into());
                self.frames.push(page);
            }
        }
    }

    /// Data: at the `offset` of the start va.
    /// Assume that all frames were cleared before.
    pub fn copy_data_with_offset(&mut self, page_table: &PageTable, offset: usize, data: &[u8]) {
        stack_trace!();
        assert_eq!(self.vma_type, VmAreaType::Elf);
        let _sum_guard = SumGuard::new();

        let mut offset = offset;
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.start();
        let len = data.len();
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE - offset)];
            let dst = &mut page_table
                .find_pte(current_vpn)
                .unwrap()
                .ppn()
                .bytes_array()[offset..offset + src.len()];
            dst.fill(0);
            dst.copy_from_slice(src);
            start += PAGE_SIZE - offset;
            offset = 0;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }
}
