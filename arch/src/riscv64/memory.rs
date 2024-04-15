pub unsafe fn flush_tlb(vaddr: usize) {
    core::arch::riscv64::sfence_vma(vaddr, 0)
}

pub unsafe fn flush_tlb_all() {
    core::arch::riscv64::sfence_vma_all()
}
