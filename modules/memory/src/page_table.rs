//! Implementation of [`PageTable`].

use alloc::{string::String, vec, vec::Vec};
use core::{iter::zip, ops::Range};

use arch::memory::switch_page_table;
use config::{
    board::MEMORY_END,
    mm::{PAGE_MASK, PAGE_SIZE, PAGE_SIZE_BITS, PTE_SIZE, VIRT_RAM_OFFSET},
};
use log::trace;
use riscv::register::satp;

use crate::{
    address::{PhysPageNum, VirtAddr, VirtPageNum},
    frame::{alloc_frame_tracker, FrameTracker},
    pte::PTEFlags,
    PageTableEntry, PhysAddr,
};

/// # Safety
///
/// Must be dropped after switching to new page table, otherwise, there will be
/// a vacuum period where satp points a waste page table since `frames` have
/// been deallocated.
pub struct PageTable {
    pub root_ppn: PhysPageNum,
    /// Frames hold all internal pages
    frames: Vec<FrameTracker>,
}

impl PageTable {
    /// Create a new empty page table.
    pub fn new() -> Self {
        let root_frame = alloc_frame_tracker();
        root_frame.fill_zero();
        PageTable {
            root_ppn: root_frame.ppn,
            frames: vec![root_frame],
        }
    }

    /// Create a page table that inherits kernel page table by shallow copying
    /// the ptes from root page table.
    ///
    /// # Safety
    ///
    /// There is only mapping from `VIRT_RAM_OFFSET`, but no MMIO mapping.
    pub fn from_kernel(kernel_page_table: &Self) -> Self {
        let root_frame = alloc_frame_tracker();
        root_frame.fill_zero();

        let kernel_start_vpn: VirtPageNum = VirtAddr::from(VIRT_RAM_OFFSET).into();
        let level_0_index = kernel_start_vpn.indices()[0];
        log::debug!(
            "[PageTable::from_kernel] kernel start vpn level 0 index {level_0_index:#x}, start vpn {kernel_start_vpn:#x}",
        );
        root_frame.ppn.pte_array()[level_0_index..]
            .copy_from_slice(&kernel_page_table.root_ppn.pte_array()[level_0_index..]);

        // the new pagetable only takes the ownership of its own root ppn
        PageTable {
            root_ppn: root_frame.ppn,
            frames: vec![root_frame],
        }
    }

    pub fn vaddr_to_paddr(vaddr: VirtAddr) -> PhysAddr {
        let satp = satp::read();
        let ppn = satp.ppn().into();
        let page_table = Self {
            root_ppn: ppn,
            frames: Vec::new(),
        };
        let leaf_pte = page_table.find_pte(vaddr.floor()).unwrap();
        let paddr = (leaf_pte.ppn().bits() << PAGE_SIZE_BITS) + (vaddr.bits() & PAGE_MASK);
        paddr.into()
    }

    /// Switch to this pagetable
    pub unsafe fn switch(&self) {
        switch_page_table(self.token());
    }

    /// Dump page table
    #[allow(unused)]
    pub fn dump(&self) {
        log::info!("----- Dump page table -----");
        self._dump(self.root_ppn, 0);
    }

    fn _dump(&self, ppn: PhysPageNum, level: usize) {
        if level >= 3 {
            return;
        }
        let mut prefix = String::from("");
        for _ in 0..level {
            prefix += "-";
        }
        for pte in ppn.pte_array() {
            if pte.is_valid() {
                log::info!("{} ppn {:#x}, flags {:?}", prefix, pte.ppn().0, pte.flags());
                self._dump(pte.ppn(), level + 1);
            }
        }
    }

    /// Find the leaf pte and will create page table in need.
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> &mut PageTableEntry {
        let idxs = vpn.indices();
        let mut ppn = self.root_ppn;
        for (i, idx) in idxs.into_iter().enumerate() {
            let pte = ppn.pte(idx);
            if i == 2 {
                return pte;
            }
            if !pte.is_valid() {
                let frame = alloc_frame_tracker();
                frame.fill_zero();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        unreachable!()
    }

    /// Find the leaf pte.
    ///
    /// Return `None` if the leaf pte is not valid.
    pub fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indices();
        let mut ppn = self.root_ppn;
        for (i, idx) in idxs.into_iter().enumerate() {
            let pte = ppn.pte(idx);
            if !pte.is_valid() {
                return None;
            }
            if i == 2 {
                return Some(pte);
            }
            ppn = pte.ppn();
        }
        return None;
    }

    /// Map `VirtPageNum` to `PhysPageNum` with `PTEFlags`.
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn);
        debug_assert!(!pte.is_valid(), "vpn {vpn:?} is mapped before mapping");
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V | PTEFlags::D | PTEFlags::A);
    }

    /// Unmap a `VirtPageNum`.
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).expect("leaf pte is not valid");
        debug_assert!(pte.is_valid(), "vpn {vpn:?} is invalid before unmapping",);
        *pte = PageTableEntry::empty();
    }

    pub fn map_kernel_region(&mut self, range_va: Range<VirtAddr>, flags: PTEFlags) {
        let range_vpn = range_va.start.floor()..range_va.end.ceil();
        for vpn in range_vpn {
            self.map(vpn, vpn.to_offset().to_ppn(), flags);
        }
    }

    pub fn map_kernel_region_offset(
        &mut self,
        range_va: Range<VirtAddr>,
        range_pa: Range<PhysAddr>,
        flags: PTEFlags,
    ) {
        let range_vpn = range_va.start.floor()..range_va.end.ceil();
        let range_ppn = range_pa.start.floor()..range_pa.end.ceil();
        for (vpn, ppn) in zip(range_vpn, range_ppn) {
            self.map(vpn, ppn, flags);
        }
    }

    /// Map the physical addresses of I/O memory resources to core virtual
    /// addresses
    ///
    /// Linux also has this function
    pub fn ioremap(&mut self, phys_addr: usize, size: usize, flags: PTEFlags) {
        let mut vpn = VirtAddr::from(phys_addr + VIRT_RAM_OFFSET).floor();
        let mut ppn = vpn.to_offset().to_ppn();
        let mut size = size as isize;
        while size > 0 {
            self.map(vpn, ppn, flags);
            vpn += 1;
            ppn += 1;
            size -= PAGE_SIZE as isize;
        }
    }

    /// Cancel the mapping made by ioremap()
    pub fn iounmap(&mut self, virt_addr: usize, size: usize) {
        let mut vpn = VirtAddr::from(virt_addr).floor();
        let mut size = size as isize;
        while size > 0 {
            self.unmap(vpn);
            vpn += 1;
            size -= PAGE_SIZE as isize;
        }
    }

    /// Force mapping `VirtPageNum` to `PhysPageNum` with `PTEFlags`.
    ///
    /// # Safety
    ///
    /// Could replace old mappings.
    pub fn map_force(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V | PTEFlags::D | PTEFlags::A);
    }

    /// Satp token with sv39 enabled
    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }

    /// Only for debug
    #[allow(unused)]
    pub fn print_page(&self, vpn: VirtPageNum) {
        use alloc::format;

        let ppn: PhysPageNum = self.find_pte(vpn).unwrap().ppn();
        log::warn!(
            "==== print page: {:x?} (in pgt {:x?}, phy: {:x?}) ====",
            vpn,
            self.root_ppn,
            ppn,
        );

        // print it 16 byte pre line
        //       0  1  2 ... f
        // 00   AC EE 12 ... 34
        // ...

        let slice = ppn.bytes_array();

        // we can only print a whole line using log::debug,
        // so we manually write it for 16 times

        log::info!("      0  1  2  3  4  5  6  7  8  9  a  b  c  d  e  f");
        for i in 0..256 {
            let mut line = format!("{:03x}   ", i * 16);
            for j in 0..16 {
                line.push_str(&format!("{:02x} ", slice[i * 16 + j]));
            }
            log::info!("{}", line);
        }

        log::warn!("==== print page done ====");
    }
}
