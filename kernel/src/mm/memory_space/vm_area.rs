use alloc::vec::Vec;
use core::ops::Range;

use config::mm::PAGE_SIZE;
use memory::{pte::PTEFlags, StepByOne, VPNRange, VirtAddr, VirtPageNum};

use super::MemorySpace;
use crate::{
    mm::{Page, PageTable},
    processor::env::SumGuard,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub struct MapPerm: u16 {
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

impl From<MapPerm> for PTEFlags {
    fn from(perm: MapPerm) -> Self {
        let mut ret = Self::from_bits(0).unwrap();
        if perm.contains(MapPerm::U) {
            ret |= PTEFlags::U;
        }
        if perm.contains(MapPerm::R) {
            ret |= PTEFlags::R;
        }
        if perm.contains(MapPerm::W) {
            ret |= PTEFlags::W;
        }
        if perm.contains(MapPerm::X) {
            ret |= PTEFlags::X;
        }
        ret
    }
}

#[derive(Debug)]
pub struct VmArea {
    pub vpn_range: VPNRange,
    pub frames: Vec<Page>,
    pub map_perm: MapPerm,
    pub vma_type: VmAreaType,
}

impl Drop for VmArea {
    fn drop(&mut self) {
        log::debug!(
            "[VmArea::drop] drop vma, [{:#x}, {:#x}]",
            self.start_vpn(),
            self.end_vpn()
        );
    }
}

impl VmArea {
    /// Construct a new vma
    ///
    /// [start_va, end_va)
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_perm: MapPerm,
        vma_type: VmAreaType,
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        log::trace!("new vpn_range: {:?}, {:?}", start_vpn, end_vpn);
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            frames: Vec::new(),
            vma_type,
            map_perm,
        }
    }

    pub fn from_another(another: &Self) -> Self {
        Self {
            vpn_range: another.vpn_range,
            frames: Vec::new(),
            vma_type: another.vma_type,
            map_perm: another.map_perm,
        }
    }

    pub fn start_va(&self) -> VirtAddr {
        self.start_vpn().into()
    }

    pub fn end_va(&self) -> VirtAddr {
        self.end_vpn().into()
    }

    pub fn range_va(&self) -> Range<VirtAddr> {
        self.start_va()..self.end_va()
    }

    pub fn start_vpn(&self) -> VirtPageNum {
        self.vpn_range.start()
    }

    pub fn end_vpn(&self) -> VirtPageNum {
        self.vpn_range.end()
    }

    /// Map `VmArea` into page table.
    ///
    /// Will alloc new pages for `VmArea` according to `VmAreaType`.
    pub fn map(&mut self, page_table: &mut PageTable) {
        if self.vma_type == VmAreaType::Physical || self.vma_type == VmAreaType::Mmio {
            for vpn in self.vpn_range {
                page_table.map(
                    vpn,
                    VirtAddr::from(vpn).to_offset().to_pa().into(),
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

    /// Copy the data to start_va + offset.
    ///
    /// # Safety
    ///
    /// Assume that all frames were cleared before.
    pub fn copy_data_with_offset(&self, page_table: &PageTable, offset: usize, data: &[u8]) {
        debug_assert_eq!(self.vma_type, VmAreaType::Elf);
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
