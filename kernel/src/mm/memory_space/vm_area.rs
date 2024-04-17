use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::sync::atomic::AtomicBool;

use arch::memory::sfence_vma_vaddr;
use config::mm::PAGE_SIZE;
use memory::{page_table, pte::PTEFlags, StepByOne, VPNRange, VirtAddr, VirtPageNum};
use spin::mutex::SpinMutex;
use sync::mutex::SpinNoIrqLock;

use super::MemorySpace;
use crate::{
    mm::{Page, PageTable},
    processor::env::SumGuard,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmAreaType {
    // For user.
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

    // For kernel.
    /// Physical frames (mapping with an offset)
    Physical,
    /// MMIO
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

        const UW = Self::U.bits() | Self::W.bits();
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
        } else {
            ret |= PTEFlags::G;
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

#[derive(Clone)]
pub struct VmArea {
    /// VPN range for the `VmArea`.
    /// NOTE: stores range that is truly allocated for lazy allocated areas.
    pub vpn_range: VPNRange,
    pub pages: BTreeMap<VirtPageNum, Arc<Page>>,
    pub map_perm: MapPerm,
    pub vma_type: VmAreaType,
}

impl core::fmt::Debug for VmArea {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VmArea")
            .field("vpn_range", &self.vpn_range)
            .field("map_perm", &self.map_perm)
            .field("vma_type", &self.vma_type)
            .finish()
    }
}

impl Drop for VmArea {
    fn drop(&mut self) {
        log::debug!("[VmArea::drop] drop {self:?}",);
        drop(self)
    }
}

impl VmArea {
    /// Construct a new vma
    pub fn new(
        range_va: core::ops::Range<VirtAddr>,
        map_perm: MapPerm,
        vma_type: VmAreaType,
    ) -> Self {
        let start_vpn: VirtPageNum = range_va.start.floor();
        let end_vpn: VirtPageNum = range_va.end.ceil();
        let new = Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            pages: BTreeMap::new(),
            vma_type,
            map_perm,
        };
        log::trace!("[VmArea::new] {new:?}");
        new
    }

    pub fn from_another(another: &Self) -> Self {
        log::trace!("[VmArea::from_another] {another:?}");
        Self {
            vpn_range: another.vpn_range,
            pages: BTreeMap::new(),
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

    pub fn range_va(&self) -> core::ops::Range<VirtAddr> {
        self.start_va()..self.end_va()
    }

    pub fn start_vpn(&self) -> VirtPageNum {
        self.vpn_range.start()
    }

    pub fn end_vpn(&self) -> VirtPageNum {
        self.vpn_range.end()
    }

    pub fn perm(&self) -> MapPerm {
        self.map_perm
    }

    pub fn get_page(&self, vpn: VirtPageNum) -> &Arc<Page> {
        self.pages.get(&vpn).expect("no page found for vpn")
    }

    /// Map `VmArea` into page table.
    ///
    /// Will alloc new pages for `VmArea` according to `VmAreaType`.
    pub fn map(&mut self, page_table: &mut PageTable) {
        // NOTE: set pte flag with global mapping for kernel memory
        let mut pte_flags: PTEFlags = self.map_perm.into();
        if self.vma_type == VmAreaType::Physical || self.vma_type == VmAreaType::Mmio {
            for vpn in self.vpn_range {
                page_table.map(
                    vpn,
                    VirtAddr::from(vpn).to_offset().to_pa().into(),
                    pte_flags,
                )
            }
        } else {
            for vpn in self.vpn_range {
                let page = Page::new();
                page_table.map(vpn, page.ppn(), pte_flags);
                self.pages.insert(vpn, Arc::new(page));
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
        while start < len {
            let src = &data[start..len.min(start + PAGE_SIZE - offset)];
            let dst = page_table
                .find_pte(current_vpn)
                .unwrap()
                .ppn()
                .bytes_array_range(offset..offset + src.len());
            dst.copy_from_slice(src);
            start += PAGE_SIZE - offset;
            offset = 0;
            current_vpn.step();
        }
    }

    pub fn handle_page_fault(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        log::debug!(
            "[VmArea::handle_page_fault]: {self:?}, {vpn:?} at page table {:?}",
            page_table.root_ppn
        );
        let page: Page;
        let pte = page_table.find_pte(vpn);
        if let Some(pte) = pte {
            // if PTE is valid, then it must be COW
            log::debug!("[VmArea::handle_page_fault] pte flags: {:?}", pte.flags());
            let mut pte_flags = pte.flags();
            debug_assert!(pte_flags.contains(PTEFlags::COW));
            debug_assert!(!pte_flags.contains(PTEFlags::W));
            debug_assert!(self.perm().contains(MapPerm::UW));

            // PERF: copying data vs. lock the area vs. atomic ref cnt
            let old_page = self.get_page(vpn);
            let mut cnt: usize;
            let cnt = Arc::strong_count(old_page);
            if cnt > 1 {
                // shared now
                log::debug!(
                    "[VmArea::handle_page_fault] copying cow page {old_page:?} with count {cnt}",
                );

                // copy the data
                page = Page::new();
                page.copy_data_from_another(&old_page);

                // unmap old page and map new page
                pte_flags.remove(PTEFlags::COW);
                pte_flags.insert(PTEFlags::W);
                page_table.map_force(vpn, page.ppn(), pte_flags);
                // NOTE: track `Page` with great care
                self.pages.insert(vpn, Arc::new(page));
                unsafe { sfence_vma_vaddr(vpn.to_va().into()) };
            } else {
                // not shared
                log::error!("[VmArea::handle_page_fault] removing cow flag for page {old_page:?}",);

                // set the pte to writable
                pte_flags.remove(PTEFlags::COW);
                pte_flags.insert(PTEFlags::W);
                pte.set_flags(pte_flags);
                unsafe { sfence_vma_vaddr(vpn.to_va().into()) };
            }
        } else {
            log::debug!(
                "[VmArea::handle_page_fault] handle for type {:?}",
                self.vma_type
            );
            match self.vma_type {
                VmAreaType::Heap => {
                    // lazy allcation for heap
                    page = Page::new();
                    page_table.map(vpn, page.ppn(), self.map_perm.into());
                    self.pages.insert(vpn, Arc::new(page));
                    unsafe { sfence_vma_vaddr(vpn.to_va().into()) };
                }
                _ => {}
            }
        }
    }
}
