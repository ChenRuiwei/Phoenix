use alloc::sync::Arc;

use config::mm::PAGE_SIZE;
use memory::{MapPermission, VirtAddr};

use super::SigSet;
use crate::{
    mm::{memory_space::vm_area::VmAreaType, Page, PageBuilder},
    process::Process,
    processor::SumGuard,
    stack_trace,
    trap::UserContext,
};
#[derive(Clone, Copy)]
#[repr(C)]
struct SignalStack {
    sp: usize,
    flags: i32,
    size: usize,
}

impl SignalStack {
    pub fn new() -> Self {
        stack_trace!();
        Self {
            sp: 0,
            flags: 0,
            size: 0,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct SigSetDummy {
    pub dummy: [usize; 15],
}

#[derive(Clone, Copy)]
#[repr(C, align(16))]
struct Align16;

#[derive(Clone)]
#[repr(C)]
pub struct SignalContext {
    flags: usize,
    link_ptr: usize,
    stack: SignalStack,
    pub blocked_sigs: SigSet,
    pub blocked_sigs_dummy: SigSetDummy,
    align16: Align16,
    pub user_context: UserContext,
}

impl SignalContext {
    pub fn new(blocked_sigs: SigSet, user_context: UserContext) -> Self {
        stack_trace!();
        Self {
            flags: 0,
            link_ptr: 0,
            stack: SignalStack::new(),
            blocked_sigs,
            blocked_sigs_dummy: SigSetDummy { dummy: [0; 15] },
            align16: Align16,
            user_context,
        }
    }
}

pub struct SignalTrampoline {
    page: Arc<Page>,
    user_addr: VirtAddr,
}

impl SignalTrampoline {
    pub fn new(process: Arc<Process>) -> Self {
        stack_trace!();
        let page = Arc::new(
            PageBuilder::new()
                .permission(MapPermission::R | MapPermission::W | MapPermission::U)
                .build(),
        );
        let permission = *page.permission.lock();
        process.inner_handler(|proc| {
            let trampoline_vma = proc
                .memory_space
                .allocate_area(PAGE_SIZE, permission, VmAreaType::Mmap)
                .unwrap();
            let user_addr: VirtAddr = trampoline_vma.start_vpn().into();
            let page_table = trampoline_vma.page_table.get_unchecked_mut();
            page_table.map(user_addr.floor(), page.data_frame.ppn, permission.into());
            proc.memory_space.insert_area(trampoline_vma);
            log::debug!(
                "[SignalTrampoline::new] map sig trampoline, vpn: {:#x}, ppn: {:#x}, flags: {:?}",
                user_addr.floor().0,
                page.data_frame.ppn.0,
                permission
            );
            Self { page, user_addr }
        })
    }

    // pub fn kernel_addr(&self) -> usize {
    //     KernelAddr::from(PhysAddr::from(self.page.data_frame.ppn)).0
    // }

    pub fn user_addr(&self) -> usize {
        stack_trace!();
        self.user_addr.0
    }

    pub fn set_signal_context(&self, signal_context: SignalContext) {
        stack_trace!();
        let _sum_guard = SumGuard::new();
        let sig_ctx: &mut SignalContext = self.page.reinterpret_mut();
        *sig_ctx = signal_context;
    }

    pub fn signal_context(&self) -> &SignalContext {
        stack_trace!();
        self.page.reinterpret()
    }
}