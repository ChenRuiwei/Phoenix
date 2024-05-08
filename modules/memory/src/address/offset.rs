use config::mm::{PAGE_SIZE, VIRT_RAM_OFFSET};

use crate::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};

/// Offset address
///
/// It is only used for kernel, which maps an area with an offset.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct OffsetAddr {
    // stores pa in usize
    pa_u: usize,
}

/// Offset page number
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct OffsetPageNum {
    ppn_u: usize,
}

impl From<PhysAddr> for OffsetAddr {
    fn from(pa: PhysAddr) -> Self {
        Self { pa_u: pa.0 }
    }
}

impl From<VirtAddr> for OffsetAddr {
    fn from(va: VirtAddr) -> Self {
        assert!(va.0 >= VIRT_RAM_OFFSET);
        Self {
            pa_u: va.0 - VIRT_RAM_OFFSET,
        }
    }
}

impl From<PhysPageNum> for OffsetPageNum {
    fn from(ppn: PhysPageNum) -> Self {
        Self { ppn_u: ppn.0 }
    }
}

impl From<VirtPageNum> for OffsetPageNum {
    fn from(vpn: VirtPageNum) -> Self {
        assert!(vpn.0 >= VIRT_RAM_OFFSET / PAGE_SIZE);
        Self {
            ppn_u: vpn.0 - VIRT_RAM_OFFSET / PAGE_SIZE,
        }
    }
}

impl OffsetAddr {
    pub fn to_pa(&self) -> PhysAddr {
        self.pa_u.into()
    }

    pub fn to_va(&self) -> VirtAddr {
        (self.pa_u + VIRT_RAM_OFFSET).into()
    }
}

impl OffsetPageNum {
    pub fn to_ppn(&self) -> PhysPageNum {
        self.ppn_u.into()
    }

    pub fn to_vpn(&self) -> VirtPageNum {
        (self.ppn_u + VIRT_RAM_OFFSET / PAGE_SIZE).into()
    }
}
