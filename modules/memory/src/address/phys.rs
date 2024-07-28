//! Implementation of physical and virtual address and page number.
use core::{
    fmt::{self},
    mem::size_of,
    ops::Range,
};

use config::mm::{PAGE_MASK, PAGE_SIZE, PAGE_SIZE_BITS, PTES_PER_PAGE, PTE_SIZE};
use crate_interface::call_interface;

use super::{impl_arithmetic_with_usize, impl_fmt, impl_step};
use crate::{
    address::{__KernelMappingIf_paddr_to_vaddr, PA_WIDTH_SV39, PPN_WIDTH_SV39},
    PageTableEntry, VirtAddr,
};

/// Physical address
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct PhysAddr(pub usize);

/// Physical page number
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysPageNum(pub usize);

impl_fmt!(PhysAddr, "PA");
impl_arithmetic_with_usize!(PhysAddr);

impl_fmt!(PhysPageNum, "PPN");
impl_step!(PhysPageNum);
impl_arithmetic_with_usize!(PhysPageNum);

impl From<usize> for PhysAddr {
    fn from(u: usize) -> Self {
        let tmp = u as isize >> PA_WIDTH_SV39;
        assert!(tmp == 0 || tmp == -1);
        Self(u)
    }
}

impl From<usize> for PhysPageNum {
    fn from(u: usize) -> Self {
        let tmp = u as isize >> PPN_WIDTH_SV39;
        assert!(tmp == 0 || tmp == -1);
        Self(u)
    }
}

impl From<PhysAddr> for usize {
    fn from(pa: PhysAddr) -> Self {
        pa.0
    }
}

impl From<PhysPageNum> for usize {
    fn from(ppn: PhysPageNum) -> Self {
        ppn.0
    }
}

impl PhysAddr {
    pub fn bits(&self) -> usize {
        self.0
    }

    /// `PhysAddr`->`PhysPageNum`
    pub fn floor(&self) -> PhysPageNum {
        PhysPageNum(self.0 / PAGE_SIZE)
    }

    /// `PhysAddr`->`PhysPageNum`
    pub fn ceil(&self) -> PhysPageNum {
        PhysPageNum((self.0 + PAGE_MASK) / PAGE_SIZE)
    }

    /// Get page offset
    pub fn page_offset(&self) -> usize {
        self.0 & PAGE_MASK
    }

    /// Check page aligned
    pub fn is_aligned(&self) -> bool {
        self.page_offset() == 0
    }

    pub fn to_vaddr(&self) -> VirtAddr {
        call_interface!(KernelMappingIf::paddr_to_vaddr(*self))
    }
}

impl From<PhysAddr> for PhysPageNum {
    fn from(pa: PhysAddr) -> Self {
        assert!(pa.is_aligned());
        pa.floor()
    }
}

impl From<PhysPageNum> for PhysAddr {
    fn from(ppn: PhysPageNum) -> Self {
        Self(ppn.0 << PAGE_SIZE_BITS)
    }
}

impl PhysPageNum {
    pub(crate) const ZERO: Self = PhysPageNum(0);

    pub fn bits(&self) -> usize {
        self.0
    }

    /// Get reference to `PhysPageNum` value
    pub fn get_ref<T>(&self) -> &'static T {
        unsafe { (self.0 as *const T).as_ref().unwrap() }
    }

    /// Get mutable reference to `PhysAddr` value
    pub fn get_mut<T>(&self) -> &'static mut T {
        unsafe { (self.0 as *mut T).as_mut().unwrap() }
    }

    pub fn to_paddr(&self) -> PhysAddr {
        (*self).into()
    }

    pub fn pte(&self, idx: usize) -> &'static mut PageTableEntry {
        let mut vaddr: VirtAddr = self.to_paddr().to_vaddr();
        vaddr += idx * PTE_SIZE;
        unsafe { vaddr.get_mut() }
    }

    /// Get `PageTableEntry` array.
    pub fn pte_array(&self) -> &'static mut [PageTableEntry] {
        let vaddr: VirtAddr = self.to_paddr().to_vaddr();
        unsafe {
            core::slice::from_raw_parts_mut(vaddr.bits() as *mut PageTableEntry, PTES_PER_PAGE)
        }
    }

    /// Get bytes array of a physical page
    pub fn bytes_array(&self) -> &'static mut [u8] {
        let vaddr: VirtAddr = self.to_paddr().to_vaddr();
        unsafe { core::slice::from_raw_parts_mut(vaddr.bits() as *mut u8, PAGE_SIZE) }
    }

    /// Get bytes array of a physical page with a range.
    pub fn bytes_array_range(&self, range: Range<usize>) -> &'static mut [u8] {
        debug_assert!(range.end <= PAGE_SIZE, "range: {range:?}");
        let mut vaddr: VirtAddr = self.to_paddr().to_vaddr();
        vaddr += range.start;
        unsafe { core::slice::from_raw_parts_mut(vaddr.bits() as *mut u8, range.len()) }
    }

    /// Empty the whole page.
    pub fn clear_page(&self) {
        self.usize_array().fill(0)
    }

    pub fn copy_page_from_another(&self, another_ppn: PhysPageNum) {
        let dst = self.usize_array();
        let src = another_ppn.usize_array();
        dst.copy_from_slice(src);
    }

    fn usize_array(&self) -> &'static mut [usize] {
        const USIZES_PER_PAGE: usize = PAGE_SIZE / size_of::<usize>();
        let vaddr: VirtAddr = self.to_paddr().to_vaddr();
        unsafe { core::slice::from_raw_parts_mut(vaddr.bits() as *mut usize, USIZES_PER_PAGE) }
    }
}
