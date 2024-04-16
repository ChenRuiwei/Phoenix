pub unsafe fn sfence_vma_vaddr(vaddr: usize) {
    core::arch::riscv64::sfence_vma_vaddr(vaddr);
}

pub unsafe fn sfence_vma_all() {
    core::arch::riscv64::sfence_vma_all();
}
