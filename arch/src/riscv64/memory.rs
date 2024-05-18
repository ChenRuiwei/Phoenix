use riscv::register::satp;

pub unsafe fn sfence_vma_vaddr(vaddr: usize) {
    core::arch::riscv64::sfence_vma_vaddr(vaddr);
}

pub unsafe fn sfence_vma_all() {
    core::arch::riscv64::sfence_vma_all();
}

/// Write `page_table_token` into satp and sfence.vma
pub unsafe fn switch_page_table(page_table_token: usize) {
    satp::write(page_table_token);
    core::arch::riscv64::sfence_vma_all();
}
