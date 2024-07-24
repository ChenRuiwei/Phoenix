    .section .text.trampoline
    .align 12
    .global _sigreturn_trampoline
_sigreturn_trampoline:
    li	a7,139
    ecall
