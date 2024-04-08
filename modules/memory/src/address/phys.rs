//! Implementation of physical and virtual address and page number.
use core::{
    fmt::{self, Debug, Formatter},
    mem::size_of,
};

use config::mm::{
    PAGE_MASK, PAGE_SIZE, PAGE_SIZE_BITS, PAGE_TABLE_LEVEL_NUM, PTE_NUM_ONE_PAGE, VIRT_RAM_OFFSET,
};

use super::{
    impl_fmt,
    offset::{OffsetAddr, OffsetPageNum},
    step::StepByOne,
};
use crate::{
    address::{PA_WIDTH_SV39, PPN_WIDTH_SV39},
    PageTableEntry, VirtAddr,
};

/// Physical address
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysAddr(pub usize);

/// Physical page number
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysPageNum(pub usize);

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
    /// `PhysAddr`->`PhysPageNum`
    pub fn floor(&self) -> PhysPageNum {
        PhysPageNum(self.0 / PAGE_SIZE)
    }
    /// `PhysAddr`->`PhysPageNum`
    pub fn ceil(&self) -> PhysPageNum {
        PhysPageNum((self.0 + PAGE_SIZE - 1) / PAGE_SIZE)
    }
    /// Get page offset
    pub fn page_offset(&self) -> usize {
        self.0 & PAGE_MASK
    }
    /// Check page aligned
    pub fn aligned(&self) -> bool {
        self.page_offset() == 0
    }
    pub fn to_offset(&self) -> OffsetAddr {
        (*self).into()
    }
}

impl From<PhysAddr> for PhysPageNum {
    fn from(pa: PhysAddr) -> Self {
        assert!(pa.aligned());
        pa.floor()
    }
}
impl From<PhysPageNum> for PhysAddr {
    fn from(ppn: PhysPageNum) -> Self {
        Self(ppn.0 << PAGE_SIZE_BITS)
    }
}

impl PhysPageNum {
    pub fn to_pa(&self) -> PhysAddr {
        (*self).into()
    }
    pub fn to_offset(&self) -> OffsetPageNum {
        (*self).into()
    }
    /// Get `PageTableEntry` on `VirtPageNum`
    pub fn pte_array(&self) -> &'static mut [PageTableEntry] {
        let va: VirtAddr = self.to_offset().to_vpn().into();
        unsafe { core::slice::from_raw_parts_mut(va.0 as *mut PageTableEntry, PTE_NUM_ONE_PAGE) }
    }
    /// Get bytes array of a physical page
    pub fn bytes_array(&self) -> &'static mut [u8] {
        let va: VirtAddr = self.to_offset().to_vpn().into();
        unsafe { core::slice::from_raw_parts_mut(va.0 as *mut u8, PAGE_SIZE) }
    }
    /// Get usize array of a physical page
    pub fn usize_array(&self) -> &'static mut [usize] {
        let va: VirtAddr = self.to_offset().to_vpn().into();
        unsafe {
            core::slice::from_raw_parts_mut(va.0 as *mut usize, PAGE_SIZE / size_of::<usize>())
        }
    }
}

impl StepByOne for PhysPageNum {
    fn step(&mut self) {
        self.0 += 1;
    }
}

impl_fmt!(PhysAddr, "PA");
impl_fmt!(PhysPageNum, "PPN");
