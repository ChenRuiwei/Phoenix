    .section .text.entry
    .globl _start
_start:
    # set by OpenSBI
    # a0 = hart id
    # pc = 0x80200000

    # set sp(each hart has one kstack)
    slli t0, a0, 16  # t0 = hart_id << 16 (hart_id * 64KB)
    la sp, boot_stack_top
    sub sp, sp, t0  # sp = stack top - hart_id * stack_size

    # since the base addr is 0xffff_ffc0_8020_0000
    # we need to activate pagetable here in case of absolute addressing
    # satp: 8 << 60 | boot_pagetable
    la t0, boot_pagetable
    li t1, 8 << 60
    srli t0, t0, 12
    or t0, t0, t1
    csrw satp, t0
    sfence.vma

    call fake_main


    .section .bss.stack
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    # 8 cpus at most
    # each kernel stack size 64KB
    .space 4096 * 16 * 8
    .globl boot_stack_top
boot_stack_top:

    .section .data
    .align 12
boot_pagetable:
    # boot_pagetable need 3 pte
    # note that mmio mapping is important for early printing in non-init harts
    # 0x0000_0000_8000_0000 -> 0x0000_0000_8000_0000
    # 0xffff_fc00_0000_0000 -> 0x0000_0000_0000_0000
    # 0xffff_fc00_8000_0000 -> 0x0000_0000_8000_0000
    .zero 8 * 2
    .quad (0x80000 << 10) | 0xcf # VRWXAD
    .zero 8 * 253
    .quad (0x00000 << 10) | 0xcf # VRWXAD
    .zero 8
    .quad (0x80000 << 10) | 0xcf # VRWXAD
    .zero 8 * 253    

    .section .text.trampoline
    .align 12
    .global sigreturn_trampoline
sigreturn_trampoline:
    li	a7,139
    ecall
