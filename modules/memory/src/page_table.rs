//! Implementation of [`PageTableEntry`] and [`PageTable`].
use alloc::{string::String, vec, vec::Vec};
use core::arch::asm;

use bitflags::*;
use config::mm::VIRT_RAM_OFFSET;
use riscv::register::satp;

// use crate::config::MMIO;
// use crate::driver::block::MMIO_VIRT;
use crate::{
    address::PhysAddr, address::PhysPageNum, address::VirtAddr, address::VirtPageNum,
    frame_allocator::frame_alloc, frame_allocator::FrameTracker,
};

/// Write `page_table_token` into satp and sfence.vma
#[inline]
pub fn activate_page_table(page_table_token: usize) {
    unsafe {
        satp::write(page_table_token);
        asm!("sfence.vma");
    }
}

bitflags! {
    /// map permission corresponding to that in pte: `R W X U`
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MapPermission: u16 {
        ///Readable
        const R = 1 << 1;
        ///Writable
        const W = 1 << 2;
        ///Excutable
        const X = 1 << 3;
        ///Accessible in U mode
        const U = 1 << 4;
        // /// COW when fork
        // const COW = 1 << 8;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PTEFlags: u16 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
        const COW = 1 << 8;
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

/// Page table entry structure
#[derive(Copy, Clone)]
#[repr(C)]
pub struct PageTableEntry {
    /// PTE
    pub bits: usize,
}

impl PageTableEntry {
    /// Create a PTE from ppn
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits() as usize,
        }
    }
    /// Return an empty PTE
    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }
    /// Return 44bit ppn
    pub fn ppn(&self) -> PhysPageNum {
        (self.bits >> 10 & ((1usize << 44) - 1)).into()
    }
    /// Return 10bit flag
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits((self.bits & ((1 << 9) - 1)) as u16).unwrap()
    }
    ///
    pub fn set_flags(&mut self, flags: PTEFlags) {
        self.bits = ((self.bits >> 10) << 10) | flags.bits() as usize;
    }
    /// Check PTE valid
    pub fn is_valid(&self) -> bool {
        self.flags().contains(PTEFlags::V)
    }
    /// Check PTE readable
    pub fn readable(&self) -> bool {
        self.flags().contains(PTEFlags::R)
    }
    /// Check PTE writable
    pub fn writable(&self) -> bool {
        self.flags().contains(PTEFlags::W)
    }
    /// Check PTE executable
    pub fn executable(&self) -> bool {
        self.flags().contains(PTEFlags::X)
    }
    /// Check PTE user access
    pub fn user_access(&self) -> bool {
        self.flags().contains(PTEFlags::U)
    }
}

///
pub struct PageTable {
    pub root_vpn: VirtPageNum,
    /// Note that these are all internal pages
    frames: Vec<FrameTracker>,
}

/// Assume that it won't oom when creating/mapping.
impl PageTable {
    /// Create a new empty pagetable
    pub fn new() -> Self {
        let root_frame = frame_alloc();
        PageTable {
            root_vpn: root_frame.vpn,
            frames: vec![root_frame],
        }
    }
    /// # Safety
    ///
    /// There is only mapping from `VIRT_RAM_OFFSET`, but no MMIO mapping
    pub fn from_global(global_root_vpn: VirtPageNum) -> Self {
        let root_frame = frame_alloc();

        // Map kernel space
        // Note that we just need shallow copy here
        let kernel_start_vpn: VirtPageNum = VirtAddr::from(VIRT_RAM_OFFSET).into();
        let level_1_index = kernel_start_vpn.indices()[0];
        log::debug!(
            "[PageTable::from_global] kernel start vpn level 1 index {:#x}, start vpn {:#x}",
            level_1_index,
            kernel_start_vpn.0
        );
        root_frame.vpn.pte_array()[level_1_index..]
            .copy_from_slice(&global_root_vpn.pte_array()[level_1_index..]);

        // the new pagetable only owns the ownership of its own root ppn
        PageTable {
            root_vpn: root_frame.vpn,
            frames: vec![root_frame],
        }
    }
    /// Switch to this pagetable
    pub fn activate(&self) {
        activate_page_table(self.token());
    }
    /// Dump page table
    #[allow(unused)]
    pub fn dump(&self) {
        log::info!("----- Dump page table -----");
        self._dump(self.root_vpn, 0);
    }
    fn _dump(&self, vpn: VirtPageNum, level: usize) {
        if level >= 3 {
            return;
        }
        let mut prefix = String::from("");
        for _ in 0..level {
            prefix += "-";
        }
        for pte in vpn.pte_array() {
            if pte.is_valid() {
                log::info!("{} ppn {:#x}, flags {:?}", prefix, pte.ppn().0, pte.flags());
                self._dump(pte.ppn().kernel_offset(), level + 1);
            }
        }
    }
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> &mut PageTableEntry {
        let idxs = vpn.indices();
        let mut vpn = self.root_vpn;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut vpn.pte_array()[*idx];
            if i == 2 {
                return pte;
            }
            if !pte.is_valid() {
                let frame = frame_alloc();
                *pte = PageTableEntry::new(frame.vpn.kernel_offset(), PTEFlags::V);
                self.frames.push(frame);
            }
            vpn = pte.ppn().kernel_offset();
        }
        unreachable!()
    }
    ///
    pub fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indices();
        let mut vpn = self.root_vpn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut vpn.pte_array()[*idx];
            if !pte.is_valid() {
                return None;
            }
            // TODO: not sure whether we should check here before return or not
            if i == 2 {
                result = Some(pte);
                break;
            }
            vpn = pte.ppn().kernel_offset();
        }
        result
    }
    ///
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn);
        if pte.is_valid() {
            log::error!("fail!!! ppn {:#x}, pte {:?}", pte.ppn().0, pte.flags());
        }
        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V | PTEFlags::D | PTEFlags::A);
    }
    /// Unmap a vpn but won't panic if not valid
    pub fn unmap_nopanic(&mut self, vpn: VirtPageNum) {
        if let Some(pte) = self.find_pte(vpn) {
            if pte.is_valid() {
                *pte = PageTableEntry::empty();
            }
        }
    }
    /// Unmap a vpn
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        *pte = PageTableEntry::empty();
        // self.frames.remove(&vpn);
    }
    ///
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }
    ///
    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.clone().floor()).map(|pte| {
            // println!("translate_va:va = {:?}", va);
            let aligned_pa: PhysAddr = pte.ppn().into();
            // println!("translate_va:pa_align = {:?}", aligned_pa);
            let offset = va.page_offset();
            let aligned_pa_usize: usize = aligned_pa.into();
            (aligned_pa_usize + offset).into()
        })
    }
    /// Satp token with sv39 enabled
    pub fn token(&self) -> usize {
        8usize << 60 | self.root_vpn.kernel_offset().0
    }
}
