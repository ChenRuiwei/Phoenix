    .section .text.trampoline
    .align 12
    .global sigreturn_trampoline
sigreturn_trampoline:
    li	a7,139
    ecall
