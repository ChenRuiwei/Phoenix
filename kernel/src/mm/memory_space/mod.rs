use alloc::collections::BTreeMap;
use config::board::MEMORY_END;
use core::cell::SyncUnsafeCell;

use log::info;
use memory::{address::SimpleRange, PageTable, VirtAddr, VirtPageNum};
use once_cell::sync::Lazy;
use sync::mutex::{SpinNoIrq, SpinNoIrqLock};

use self::vm_area::VmArea;
use crate::utils::stack_trace;

///
pub mod page_fault_handler;
///
pub mod vm_area;

mod cow;

extern "C" {
    fn _stext();
    fn _strampoline();
    fn sigreturn_trampoline();
    fn _etrampoline();
    fn _etext();
    fn _srodata();
    fn _erodata();
    fn _sdata();
    fn _edata();
    fn _sstack();
    fn _estack();
    fn _sbss();
    fn _ebss();
    fn _ekernel();
}

/// Kernel Space for all processes
pub static mut KERNEL_SPACE: Lazy<SpinNoIrqLock<MemorySpace>> =
    Lazy::new(|| SpinNoIrqLock::new(MemorySpace::new_kernel()));

pub fn activate_kernel_space() {
    unsafe {
        KERNEL_SPACE
            .as_ref()
            .expect("KERNEL SPACE not init yet")
            .activate();
    }
}

/// memory space structure, controls virtual-memory space
pub struct MemorySpace {
    pub page_table: SyncUnsafeCell<PageTable>,
    /// start vpn -> vm_area
    areas: SyncUnsafeCell<BTreeMap<VirtPageNum, VmArea>>,
}

impl MemorySpace {
    /// Create an empty `MemorySpace`
    pub fn new_bare() -> Self {
        stack_trace!();
        let page_table = SyncUnsafeCell::new(PageTable::new());
        Self {
            page_table,
            areas: SyncUnsafeCell::new(BTreeMap::new()),
        }
    }

    /// Create a kernel space
    pub fn new_kernel() -> Self {
        stack_trace!();
        let mut memory_space = Self::new_bare();
        info!("[kernel] trampoline {:#x}", sigreturn_trampoline as usize);
        // // map trampoline
        // memory_space.map_trampoline();
        // map kernel sections
        info!(
            "[kernel].text [{:#x}, {:#x}) [{:#x}, {:#x})",
            _stext as usize, _strampoline as usize, _etrampoline as usize, _etext as usize
        );
        info!(
            "[kernel].text.trampoline [{:#x}, {:#x})",
            _strampoline as usize, _etrampoline as usize,
        );
        info!(
            "[kernel].rodata [{:#x}, {:#x})",
            _srodata as usize, _erodata as usize
        );
        info!(
            "[kernel].data [{:#x}, {:#x})",
            _sdata as usize, _edata as usize
        );
        info!(
            "[kernel].stack [{:#x}, {:#x})",
            _sstack as usize, _estack as usize
        );
        info!(
            "[kernel].bss [{:#x}, {:#x})",
            _sbss as usize, _ebss as usize
        );
        info!(
            "[kernel]physical mem [{:#x}, {:#x})",
            _ekernel as usize, MEMORY_END as usize
        );

        info!("[kernel]mapping .text section");
        memory_space.push(
            VmArea::new(
                (_stext as usize).into(),
                (_strampoline as usize).into(),
                MapType::Direct,
                MapPermission::R | MapPermission::X,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        memory_space.push(
            VmArea::new(
                (_etrampoline as usize).into(),
                (_etext as usize).into(),
                MapType::Direct,
                MapPermission::R | MapPermission::X,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        info!("[kernel]mapping .rodata section");
        memory_space.push(
            VmArea::new(
                (_srodata as usize).into(),
                (_erodata as usize).into(),
                MapType::Direct,
                MapPermission::R,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        info!("[kernel]mapping .data section");
        memory_space.push(
            VmArea::new(
                (_sdata as usize).into(),
                (_edata as usize).into(),
                MapType::Direct,
                MapPermission::R | MapPermission::W,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        // add stack section in `linker.ld`
        info!("[kernel]mapping .stack section");
        memory_space.push(
            VmArea::new(
                (_sstack as usize).into(),
                (_estack as usize).into(),
                MapType::Direct,
                MapPermission::R | MapPermission::W,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        info!("[kernel]mapping .bss section");
        memory_space.push(
            VmArea::new(
                (_sbss as usize).into(),
                (_ebss as usize).into(),
                MapType::Direct,
                MapPermission::R | MapPermission::W,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        info!("[kernel]mapping signal-return trampoline");
        memory_space.push(
            VmArea::new(
                (_strampoline as usize).into(),
                (_etrampoline as usize).into(),
                MapType::Direct,
                MapPermission::R | MapPermission::X | MapPermission::U,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        // info!("{:#x}", unsafe { *(strampoline as usize as *const usize) });
        info!("[kernel]mapping physical memory");
        memory_space.push(
            VmArea::new(
                (_ekernel as usize).into(),
                MEMORY_END.into(),
                MapType::Direct,
                MapPermission::R | MapPermission::W,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Physical,
            ),
            0,
            None,
        );
        info!("[kernel] mapping mmio registers");
        for pair in MMIO {
            info!("start va: {:#x}", pair.0);
            info!("end va: {:#x}", pair.0 + pair.1);
            info!("permission: {:?}", pair.2);
            memory_space.push(
                VmArea::new(
                    (pair.0 + VIRT_RAM_OFFSET).into(),
                    (pair.0 + pair.1 + VIRT_RAM_OFFSET).into(),
                    MapType::Direct,
                    pair.2,
                    None,
                    None,
                    memory_space.page_table.clone(),
                    VmAreaType::Mmio,
                ),
                0,
                None,
            );
        }
        info!("[kernel] new kernel finished");
        memory_space
    }

    /// Map the sections in the elf.
    /// Return the max end vpn and the first section's va.
    fn map_elf(
        &mut self,
        elf: &ElfFile,
        elf_file: Option<&Arc<dyn File>>,
        offset: VirtAddr,
    ) -> (VirtPageNum, VirtAddr) {
        stack_trace!();
        let elf_header = elf.header;
        let ph_count = elf_header.pt2.ph_count();

        let mut max_end_vpn = offset.floor();
        let mut header_va = 0;
        let mut has_found_header_va = false;
        info!("[map_elf]: entry point {:#x}", elf.header.pt2.entry_point());

        let page_cache = elf_file.map(|file| {
            file.metadata()
                .inner
                .lock()
                .inode
                .as_ref()
                .unwrap()
                .metadata()
                .inner
                .lock()
                .page_cache
                .as_ref()
                .unwrap()
                .clone()
        });

        // self.cow_pages.clear();
        // self.areas.get_mut().clear();
        // self.page_table.get_unchecked_mut().clear_user_space();

        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() != xmas_elf::program::Type::Load {
                continue;
            }
            let start_va: VirtAddr = (ph.virtual_addr() as usize + offset.0).into();
            let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize + offset.0).into();
            if !has_found_header_va {
                header_va = start_va.0;
                has_found_header_va = true;
            }
            let mut map_perm = MapPermission::U;
            let ph_flags = ph.flags();
            if ph_flags.is_read() {
                map_perm |= MapPermission::R;
            }
            if ph_flags.is_write() {
                map_perm |= MapPermission::W;
            }
            if ph_flags.is_execute() {
                map_perm |= MapPermission::X;
            }
            let mut vm_area = VmArea::new(
                start_va,
                end_va,
                MapType::Framed,
                map_perm,
                None,
                None,
                self.page_table.clone(),
                VmAreaType::Elf,
            );

            log::debug!(
                "[map_elf] [{:#x}, {:#x}], map_perm: {:?} start...",
                start_va.0,
                end_va.0,
                map_perm
            );

            max_end_vpn = vm_area.vpn_range.end();

            let map_offset = start_va.0 - start_va.floor().0 * PAGE_SIZE;

            log::debug!(
                "[map_elf] ph offset {:#x}, file size {:#x}, mem size {:#x}",
                ph.offset(),
                ph.file_size(),
                ph.mem_size()
            );
            if !map_perm.contains(MapPermission::W) && page_cache.is_some() {
                log::debug!(
                    "[map_elf] map shared page: [{:#x}, {:#x}]",
                    vm_area.start_vpn().0,
                    vm_area.end_vpn().0
                );
                let mut file_offset = ph.offset() as usize;
                for vpn in vm_area.vpn_range {
                    let page = page_cache
                        .as_ref()
                        .unwrap()
                        .get_page(file_offset, Some(map_perm))
                        .unwrap();
                    *page.permission.lock() = map_perm;
                    vm_area.map_one(vpn, Some(page));
                    file_offset += PAGE_SIZE;
                }
                self.push_lazily(vm_area, None);
            } else {
                self.push(
                    vm_area,
                    map_offset,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }

            info!(
                "[map_elf] [{:#x}, {:#x}], map_perm: {:?}",
                start_va.0, end_va.0, map_perm
            );
        }

        (max_end_vpn, header_va.into())
    }

    /// Include sections in elf and TrapContext and user stack,
    /// also returns user_sp and entry point.
    /// TODO: resolve elf file lazily
    pub fn from_elf(
        elf_data: &[u8],
        elf_file: Option<&Arc<dyn File>>,
    ) -> (Self, usize, usize, Vec<AuxHeader>) {
        stack_trace!();
        let mut memory_space = Self::new_from_global();

        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        assert_eq!(
            elf_header.pt1.magic,
            [0x7f, 0x45, 0x4c, 0x46],
            "invalid elf!"
        );
        let mut entry_point = elf_header.pt2.entry_point() as usize;
        let ph_entry_size = elf_header.pt2.ph_entry_size() as usize;
        let ph_count = elf_header.pt2.ph_count() as usize;

        let mut auxv = generate_early_auxv(ph_entry_size, ph_count, entry_point);
        if let Some(interp_entry_point) = memory_space.load_dl_interp_if_needed(&elf) {
            auxv.push(AuxHeader::new(AT_BASE, DL_INTERP_OFFSET));
            entry_point = interp_entry_point;
        } else {
            auxv.push(AuxHeader::new(AT_BASE, 0));
        }

        let (max_end_vpn, header_va) = memory_space.map_elf(&elf, elf_file, 0.into());

        let ph_head_addr = header_va.0 + elf.header.pt2.ph_offset() as usize;
        debug!("[from_elf] AT_PHDR  ph_head_addr is {:x} ", ph_head_addr);
        auxv.push(AuxHeader::new(AT_PHDR, ph_head_addr));

        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_bottom: usize = max_end_va.into();
        // guard page
        user_stack_bottom += PAGE_SIZE;

        // We will add the ustack to memory set later by `Thread` itself
        // Now we add heap section
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;

        let ustack_vma = VmArea::new(
            user_stack_bottom.into(),
            user_stack_top.into(),
            MapType::Framed,
            MapPermission::U | MapPermission::R | MapPermission::W,
            Some(Arc::new(UStackPageFaultHandler {})),
            None,
            memory_space.page_table.clone(),
            VmAreaType::Stack,
        );
        memory_space.push_lazily(ustack_vma, None);
        log::info!(
            "[from_elf] map ustack: {:#x}, {:#x}",
            user_stack_bottom,
            user_stack_top,
        );

        // guard page
        let heap_start_va = user_stack_top + PAGE_SIZE;
        let heap_end_va = heap_start_va;
        let map_perm = MapPermission::U | MapPermission::R | MapPermission::W;
        let heap_vma = VmArea::new(
            heap_start_va.into(),
            heap_end_va.into(),
            MapType::Framed,
            map_perm,
            Some(Arc::new(SBrkPageFaultHandler {})),
            None,
            memory_space.page_table.clone(),
            VmAreaType::Brk,
        );
        memory_space.push(heap_vma, 0, None);
        memory_space.heap_range = Some(HeapRange::new(heap_start_va.into(), heap_end_va.into()));
        log::info!(
            "[from_elf] map heap: {:#x}, {:#x}",
            heap_start_va,
            heap_end_va
        );

        (memory_space, user_stack_top, entry_point, auxv)
    }

    /// Check whether the elf file is dynamic linked and
    /// if so, load the dl interpreter.
    /// Return the interpreter's entry point(at the base of DL_INTERP_OFFSET) if
    /// so.
    fn load_dl_interp_if_needed(&mut self, elf: &ElfFile) -> Option<usize> {
        stack_trace!();
        let elf_header = elf.header;
        let ph_count = elf_header.pt2.ph_count();

        let mut is_dl = false;
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Interp {
                is_dl = true;
                break;
            }
        }

        if is_dl {
            log::info!("[load_dl] encounter a dl elf");
            let section = elf.find_section_by_name(".interp").unwrap();
            let mut interp = String::from_utf8(section.raw_data(&elf).to_vec()).unwrap();
            interp = interp.strip_suffix("\0").unwrap_or(&interp).to_string();
            log::info!("[load_dl] interp {}", interp);

            let mut interps: Vec<String> = vec![interp.clone()];

            log::info!("interp {}", interp);

            if interp.eq("/lib/ld-musl-riscv64-sf.so.1") || interp.eq("/lib/ld-musl-riscv64.so.1") {
                // interp = "/lib/libc.so".to_string();
                interps.push("/libc.so".to_string());
                interps.push("/lib/libc.so".to_string());
            }

            let mut interp_inode = None;
            for interp in interps {
                if let Some(inode) = resolve_path(AT_FDCWD, &interp, OpenFlags::RDONLY).ok() {
                    interp_inode = Some(inode);
                    break;
                }
            }
            let interp_inode = interp_inode.unwrap();
            let interp_file = interp_inode.open(interp_inode.clone()).ok().unwrap();
            let mut interp_elf_data = Vec::new();
            interp_file
                .read_all_from_start(&mut interp_elf_data)
                .ok()
                .unwrap();
            let interp_elf = xmas_elf::ElfFile::new(&interp_elf_data).unwrap();
            self.map_elf(&interp_elf, Some(&interp_file), DL_INTERP_OFFSET.into());

            Some(interp_elf.header.pt2.entry_point() as usize + DL_INTERP_OFFSET)
        } else {
            debug!("[load_dl] encounter a static elf");
            None
        }
    }
}
