// use config::mm::{KERNEL_STACK_SIZE, VIRT_RAM_OFFSET};
//
// #[link_section = ".bss.stack"]
// static mut STACK: [u8; KERNEL_STACK_SIZE * 8] = [0u8; KERNEL_STACK_SIZE * 8];
//
// static mut PAGE_TABLE: [u64; 512] = {
//     let mut arr: [u64; 512] = [0; 512];
//     arr[2] = (0x80000 << 10) | 0xcf;
//     arr[256] = (0x00000 << 10) | 0xcf;
//     arr[258] = (0x80000 << 10) | 0xcf;
//     arr
// };
//
// #[naked]
// #[no_mangle]
// #[link_section = ".text.entry"]
// unsafe extern "C" fn _start(hart_id: usize) -> ! {
//     core::arch::asm!(
//         // 1. set boot stack
//         // sp = boot_stack + (hartid + 1) * 64KB
//         "
//             addi    t0, a0, 1
//             slli    t0, t0, 16              // t0 = (hart_id + 1) * 64KB
//             la      sp, {boot_stack}
//             add     sp, sp, t0              // set boot stack
//         ",
//         // 2. enable sv39 page table
//         // satp = (8 << 60) | PPN(page_table)
//         "
//             la      t0, {page_table}
//             srli    t0, t0, 12
//             li      t1, 8 << 60
//             or      t0, t0, t1
//             csrw    satp, t0
//             sfence.vma
//         ",
//         // 3. jump to rust_main
//         // add virtual address offset to sp and pc
//         "
//             li      t2, {virt_ram_offset}
//             or      sp, sp, t2
//             la      a2, rust_main
//             or      a2, a2, t2
//             jalr    a2                      // call rust_main
//         ",
//         boot_stack = sym STACK,
//         page_table = sym PAGE_TABLE,
//         virt_ram_offset = const VIRT_RAM_OFFSET,
//         options(noreturn),
//     )
// }
