use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use core::ops::{Range, RangeBounds};

use arch::{memory::sfence_vma_vaddr, sstatus};
use async_utils::block_on;
use config::mm::{align_offset_to_page, round_down_to_page, PAGE_SIZE};
use memory::{pte::PTEFlags, VirtAddr, VirtPageNum};
use page::Page;
use systype::{SysError, SysResult};
use vfs_core::File;

use crate::{
    mm::{PageFaultAccessType, PageTable, UserSlice},
    processor::env::SumGuard,
    syscall::MmapFlags,
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

impl From<PTEFlags> for MapPerm {
    fn from(flags: PTEFlags) -> Self {
        let mut ret = Self::from_bits(0).unwrap();
        if flags.contains(PTEFlags::U) {
            ret |= MapPerm::U
        }
        if flags.contains(PTEFlags::R) {
            ret |= MapPerm::R;
        }
        if flags.contains(PTEFlags::W) {
            ret |= MapPerm::W;
        }
        if flags.contains(PTEFlags::X) {
            ret |= MapPerm::X;
        }
        ret
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

/// A contiguous virtual memory area.
#[derive(Clone)]
pub struct VmArea {
    /// Aligned `VirtAddr` range for the `VmArea`.
    range_va: Range<VirtAddr>,
    /// Hold pages with RAII.
    pub pages: BTreeMap<VirtPageNum, Arc<Page>>,
    /// Map permission of this area.
    pub map_perm: MapPerm,
    /// Type of this area.
    pub vma_type: VmAreaType,

    // For mmap.
    /// Mmap flags.
    pub mmap_flags: MmapFlags,
    /// The underlying file being mapped.
    pub backed_file: Option<Arc<dyn File>>,
    /// Start offset in the file.
    pub offset: usize,
}

impl core::fmt::Debug for VmArea {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VmArea")
            .field("range_va", &self.range_va)
            .field("map_perm", &self.map_perm)
            .field("vma_type", &self.vma_type)
            .finish()
    }
}

impl Drop for VmArea {
    fn drop(&mut self) {
        log::debug!("[VmArea::drop] drop {self:?}",);
    }
}

impl VmArea {
    /// Construct a new vma.
    pub fn new(range_va: Range<VirtAddr>, map_perm: MapPerm, vma_type: VmAreaType) -> Self {
        let range_va = range_va.start.floor().into()..range_va.end.ceil().into();
        let new = Self {
            range_va,
            pages: BTreeMap::new(),
            vma_type,
            map_perm,
            backed_file: None,
            mmap_flags: MmapFlags::default(),
            offset: 0,
        };
        log::debug!("[VmArea::new] {new:?}");
        new
    }

    pub fn new_mmap(
        range_va: Range<VirtAddr>,
        map_perm: MapPerm,
        mmap_flags: MmapFlags,
        file: Option<Arc<dyn File>>,
        offset: usize,
    ) -> Self {
        let range_va = range_va.start.floor().into()..range_va.end.ceil().into();
        let new = Self {
            range_va,
            pages: BTreeMap::new(),
            vma_type: VmAreaType::Mmap,
            map_perm,
            backed_file: file,
            mmap_flags,
            offset,
        };
        log::debug!("[VmArea::new_mmap] {new:?}");
        new
    }

    pub fn from_another(another: &Self) -> Self {
        log::debug!("[VmArea::from_another] {another:?}");
        Self {
            range_va: another.range_va(),
            pages: BTreeMap::new(),
            vma_type: another.vma_type,
            map_perm: another.map_perm,
            backed_file: another.backed_file.clone(),
            mmap_flags: another.mmap_flags,
            offset: another.offset,
        }
    }

    pub fn start_va(&self) -> VirtAddr {
        self.range_va().start
    }

    pub fn end_va(&self) -> VirtAddr {
        self.range_va().end
    }

    pub fn start_vpn(&self) -> VirtPageNum {
        self.start_va().into()
    }

    pub fn end_vpn(&self) -> VirtPageNum {
        self.end_va().into()
    }

    pub fn range_va(&self) -> Range<VirtAddr> {
        self.range_va.clone()
    }

    pub fn range_vpn(&self) -> Range<VirtPageNum> {
        self.start_vpn()..self.end_vpn()
    }

    pub fn set_range_va(&mut self, range_va: Range<VirtAddr>) {
        self.range_va = range_va
    }

    pub fn perm(&self) -> MapPerm {
        self.map_perm
    }

    pub fn set_perm(&mut self, perm: MapPerm) {
        self.map_perm = perm;
    }

    pub fn get_page(&self, vpn: VirtPageNum) -> &Arc<Page> {
        self.pages.get(&vpn).expect("no page found for vpn")
    }

    pub fn fill_zero(&self) {
        for page in self.pages.values() {
            page.fill_zero()
        }
    }

    pub fn set_perm_and_flush(&mut self, page_table: &mut PageTable, perm: MapPerm) {
        self.set_perm(perm);
        let pte_flags = perm.into();
        let range_vpn = self.range_vpn();
        // NOTE: should flush pages that already been allocated, page fault handler will
        // handle the permission of those unallocated pages
        for &vpn in self.pages.keys() {
            let pte = page_table.find_pte(vpn).unwrap();
            log::trace!(
                "[origin pte:{:?}, new_flag:{:?}]",
                pte.flags(),
                pte.flags().union(pte_flags)
            );
            pte.set_flags(pte.flags().union(pte_flags));
            unsafe { sfence_vma_vaddr(vpn.to_va().into()) };
        }
    }

    pub fn flush(&mut self, page_table: &mut PageTable) {
        let range_vpn = self.range_vpn();
        for vpn in range_vpn {
            unsafe { sfence_vma_vaddr(vpn.to_va().into()) };
        }
    }

    /// Map `VmArea` into page table.
    ///
    /// Will alloc new pages for `VmArea` according to `VmAreaType`.
    pub fn map(&mut self, page_table: &mut PageTable) {
        // NOTE: set pte flag with global mapping for kernel memory
        let pte_flags: PTEFlags = self.map_perm.into();

        for vpn in self.range_vpn() {
            let page = Page::new();
            // page.clear();
            page_table.map(vpn, page.ppn(), pte_flags);
            self.pages.insert(vpn, page);
        }
    }

    pub fn map_range(&mut self, page_table: &mut PageTable, range: Range<VirtAddr>) {
        let range_vpn = range.start.into()..range.end.into();
        assert!(self.start_vpn() <= range_vpn.start && self.end_vpn() >= range_vpn.end);
        let pte_flags: PTEFlags = self.map_perm.into();
        for vpn in range_vpn {
            let page = Page::new();
            page_table.map(vpn, page.ppn(), pte_flags);
            unsafe { sfence_vma_vaddr(vpn.to_va().into()) };
            self.pages.insert(vpn, page);
        }
    }

    pub fn unmap(&mut self, page_table: &mut PageTable) {
        let vpns: Vec<_> = self.pages.keys().cloned().collect();
        for vpn in vpns {
            page_table.unmap(vpn);
            unsafe { sfence_vma_vaddr(vpn.to_va().into()) };
            self.pages.remove(&vpn);
        }
    }

    /// Copy the data to start_va + offset.
    ///
    /// # Safety
    ///
    /// Assume that all frames were cleared before.
    // HACK: ugly
    pub fn copy_data_with_offset(
        &mut self,
        page_table: &mut PageTable,
        offset: usize,
        data: &[u8],
    ) {
        // debug_assert_eq!(self.vma_type, VmAreaType::Elf);
        let _sum_guard = SumGuard::new();

        let mut offset = offset;
        let mut start: usize = 0;
        let mut current_vpn = self.start_vpn();
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
            current_vpn += 1;
        }
    }

    pub fn split(
        mut self,
        split_range: Range<VirtAddr>,
    ) -> (Option<Self>, Option<Self>, Option<Self>) {
        debug_assert!(split_range.start.is_aligned() && split_range.end.is_aligned());
        debug_assert!(split_range.start >= self.start_va() && split_range.end <= self.end_va());
        let start_vpn: VirtPageNum = split_range.start.into();
        let end_vpn: VirtPageNum = split_range.end.into();
        let (mut left, mut middle, mut right) = (None, None, None);
        let (left_range, middle_range, right_range) = (
            self.start_va()..split_range.start,
            split_range.clone(),
            split_range.end..self.end_va(),
        );
        if !left_range.is_empty() {
            let mut left_vma = VmArea::from_another(&self);
            left_vma.set_range_va(left_range);
            left_vma.pages.extend(
                self.pages
                    .range(left_vma.range_vpn())
                    .into_iter()
                    .map(|(&k, v)| (k, v.clone())),
            );
            left_vma.offset += left_vma.start_va() - self.start_va();
            left = Some(left_vma)
        }
        if !middle_range.is_empty() {
            let mut middle_vma = VmArea::from_another(&self);
            middle_vma.set_range_va(middle_range);
            middle_vma.pages.extend(
                self.pages
                    .range(middle_vma.range_vpn())
                    .into_iter()
                    .map(|(&k, v)| (k, v.clone())),
            );
            middle_vma.offset += middle_vma.start_va() - self.start_va();
            middle = Some(middle_vma)
        }
        if !right_range.is_empty() {
            let mut right_vma = VmArea::from_another(&self);
            right_vma.set_range_va(right_range);
            right_vma.pages.extend(
                self.pages
                    .range(right_vma.range_vpn())
                    .into_iter()
                    .map(|(&k, v)| (k, v.clone())),
            );
            right_vma.offset += right_vma.start_va() - self.start_va();
            right = Some(right_vma)
        }
        log::info!("[VmArea::split] left: {left:?}");
        log::info!("[VmArea::split] middle: {middle:?}");
        log::info!("[VmArea::split] right: {right:?}");
        (left, middle, right)
    }

    // FIXME: should kill user program if it deref a invalid pointer, e.g. try to
    // write at a read only area?
    pub fn handle_page_fault(
        &mut self,
        page_table: &mut PageTable,
        vpn: VirtPageNum,
        access_type: PageFaultAccessType,
    ) -> SysResult<()> {
        log::debug!(
            "[VmArea::handle_page_fault] {self:?}, {vpn:?} at page table {:?}",
            page_table.root_ppn
        );

        if !access_type.can_access(self.perm()) {
            log::warn!(
                "[VmArea::handle_page_fault] permission not allowed, perm:{:?}",
                self.perm()
            );
            return Err(SysError::EFAULT);
        }

        let page: Arc<Page>;
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
            let cnt = Arc::strong_count(old_page);
            if cnt > 1 {
                // shared now
                log::debug!(
                    "[VmArea::handle_page_fault] copying cow page {old_page:?} with count {cnt}",
                );

                // copy the data
                page = Page::new();
                page.copy_from_slice(old_page.bytes_array());

                // unmap old page and map new page
                pte_flags.remove(PTEFlags::COW);
                pte_flags.insert(PTEFlags::W);
                page_table.map_force(vpn, page.ppn(), pte_flags);
                // NOTE: track `Page` with great care
                self.pages.insert(vpn, page);
                unsafe { sfence_vma_vaddr(vpn.to_va().into()) };
            } else {
                // not shared
                log::debug!("[VmArea::handle_page_fault] removing cow flag for page {old_page:?}",);

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
                VmAreaType::Heap | VmAreaType::Stack => {
                    // lazy allcation for heap
                    page = Page::new();
                    page.fill_zero();
                    page_table.map(vpn, page.ppn(), self.map_perm.into());
                    self.pages.insert(vpn, page);
                    unsafe { sfence_vma_vaddr(vpn.to_va().into()) };
                }
                VmAreaType::Mmap => {
                    if !self.mmap_flags.contains(MmapFlags::MAP_ANONYMOUS) {
                        // file mapping
                        let file = self.backed_file.as_ref().unwrap();
                        let offset = self.offset + (vpn - self.start_vpn()) * PAGE_SIZE;
                        let offset_aligned = round_down_to_page(offset);
                        if self.mmap_flags.contains(MmapFlags::MAP_SHARED) {
                            let page = block_on(async { file.get_page_at(offset_aligned).await })?
                                .unwrap();
                            page_table.map(vpn, page.ppn(), self.map_perm.into());
                            self.pages.insert(vpn, page);
                            unsafe { sfence_vma_vaddr(vpn.to_va().into()) };
                        } else {
                            let page = block_on(async { file.get_page_at(offset_aligned).await })?
                                .unwrap();
                            let (pte_flags, ppn) = {
                                let mut new_flags: PTEFlags = self.map_perm.into();
                                new_flags |= PTEFlags::COW;
                                new_flags.remove(PTEFlags::W);
                                (new_flags, page.ppn())
                            };
                            page_table.map(vpn, ppn, pte_flags);
                            self.pages.insert(vpn, page);
                            unsafe { sfence_vma_vaddr(vpn.to_va().into()) };
                        }
                    } else if self.mmap_flags.contains(MmapFlags::MAP_PRIVATE) {
                        if self.mmap_flags.contains(MmapFlags::MAP_SHARED) {
                            todo!()
                        } else {
                            // private anonymous area
                            page = Page::new();
                            page.fill_zero();
                            page_table.map(vpn, page.ppn(), self.map_perm.into());
                            self.pages.insert(vpn, page);
                            unsafe { sfence_vma_vaddr(vpn.to_va().into()) };
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
