//! Implementation of physical and virtual address and page number.
use core::{
    fmt::{self, Debug, Formatter},
    mem::size_of,
};

use config::mm::{
    PAGE_MASK, PAGE_SIZE, PAGE_SIZE_BITS, PAGE_TABLE_LEVEL_NUM, PTE_NUM_ONE_PAGE, VIRT_RAM_OFFSET,
};

use super::{
    impl_arithmetic_with_usize, impl_fmt,
    offset::OffsetAddr,
    step::{SimpleRange, StepByOne},
};
use crate::address::{VA_WIDTH_SV39, VPN_WIDTH_SV39};

/// Virtual address
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtAddr(pub usize);

/// Virtual page number
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtPageNum(pub usize);

impl_fmt!(VirtAddr, "VA");
impl_fmt!(VirtPageNum, "VPN");
impl_arithmetic_with_usize!(VirtPageNum);
impl_arithmetic_with_usize!(VirtAddr);

impl From<usize> for VirtAddr {
    fn from(v: usize) -> Self {
        let tmp = v as isize >> VA_WIDTH_SV39;
        assert!(tmp == 0 || tmp == -1, "invalid va: {:#x}", v);
        Self(v)
    }
}
impl From<usize> for VirtPageNum {
    fn from(v: usize) -> Self {
        let tmp = v >> (VPN_WIDTH_SV39 - 1);
        assert!(tmp == 0 || tmp == (1 << (52 - VPN_WIDTH_SV39 + 1)) - 1);
        Self(v)
    }
}

impl From<VirtAddr> for usize {
    fn from(v: VirtAddr) -> Self {
        if v.0 >= (1 << (VA_WIDTH_SV39 - 1)) {
            v.0 | (!((1 << VA_WIDTH_SV39) - 1))
        } else {
            v.0
        }
    }
}

impl From<VirtPageNum> for usize {
    fn from(v: VirtPageNum) -> Self {
        v.0
    }
}

impl VirtAddr {
    pub const fn from_usize(v: usize) -> Self {
        Self(v)
    }
    pub const fn bits(&self) -> usize {
        self.0
    }
    pub fn to_offset(&self) -> OffsetAddr {
        (*self).into()
    }
    /// `VirtAddr`->`VirtPageNum`
    pub fn floor(&self) -> VirtPageNum {
        VirtPageNum(self.0 / PAGE_SIZE)
    }
    /// `VirtAddr`->`VirtPageNum`
    pub fn ceil(&self) -> VirtPageNum {
        VirtPageNum((self.0 + PAGE_SIZE - 1) / PAGE_SIZE)
    }
    /// Get page offset
    pub fn page_offset(&self) -> usize {
        self.0 & PAGE_MASK
    }
    /// Check page aligned
    pub fn aligned(&self) -> bool {
        self.page_offset() == 0
    }
    pub const fn as_ptr(self) -> *const u8 {
        self.0 as *const u8
    }
    pub const fn as_mut_ptr(self) -> *mut u8 {
        self.0 as *mut u8
    }
}
impl From<VirtAddr> for VirtPageNum {
    fn from(v: VirtAddr) -> Self {
        assert_eq!(v.page_offset(), 0);
        v.floor()
    }
}
impl From<VirtPageNum> for VirtAddr {
    fn from(v: VirtPageNum) -> Self {
        Self(v.0 << PAGE_SIZE_BITS)
    }
}

impl VirtPageNum {
    pub fn to_va(&self) -> VirtAddr {
        (*self).into()
    }
    pub fn next(&self) -> Self {
        *self + 1
    }
    /// Return VPN 3 level indices
    pub fn indices(&self) -> [usize; PAGE_TABLE_LEVEL_NUM] {
        let mut vpn = self.0;
        let mut indices = [0usize; PAGE_TABLE_LEVEL_NUM];
        for i in (0..PAGE_TABLE_LEVEL_NUM).rev() {
            indices[i] = vpn & 511;
            vpn >>= 9;
        }
        indices
    }
}

impl StepByOne for VirtAddr {
    fn step(&mut self) {
        self.0 += 1;
    }
}

impl StepByOne for VirtPageNum {
    fn step(&mut self) {
        self.0 += 1;
    }
}

/// a simple range structure for virtual page number
pub type VPNRange = SimpleRange<VirtPageNum>;
