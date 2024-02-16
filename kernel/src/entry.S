    .section .text.entry
    .globl _start
_start:
    la sp, boot_stack_top

    la t0, boot_pagetable
    # set mode to sv39
    li t1, 8 << 60
    # ppn of boot_pagetable
    srli t0, t0, 12
    or t0, t0, t1
    csrw satp, t0
    sfence.vma

    lla t2, rust_main
    jalr zero, 0(t2)


    .section .bss.stack
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top:


    .section .data
    .align 12
boot_pagetable:
    # a root pagetable which contains the next two pte
    # 0x0000_0000_8000_0000 -> 0x0000_0000_8000_0000
    # 0xffff_fc00_8000_0000 -> 0x0000_0000_8000_0000
    .zero 8 * 2
    .8byte (0x80000 << 10) | 0xcf # VRWXAD
    .zero 8 * 255
    .8byte (0x80000 << 10) | 0xcf # VRWXAD
    .zero 8 * 253
