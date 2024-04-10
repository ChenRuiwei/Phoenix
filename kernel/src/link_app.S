
    .align 3
    .section .data
    .global _num_app
_num_app:
    .quad 5
    .quad app_0_start
    .quad app_1_start
    .quad app_2_start
    .quad app_3_start
    .quad app_4_start
    .quad app_4_end

    .global _app_names
_app_names:
    .string "hello_world"
    .string "initproc"
    .string "runtestcase"
    .string "shell"
    .string "busybox"

    .section .data
    .global app_0_start
    .global app_0_end
    .align 3
app_0_start:
    .incbin "./target/riscv64gc-unknown-none-elf/debug/hello_world"
app_0_end:

    .section .data
    .global app_1_start
    .global app_1_end
    .align 3
app_1_start:
    .incbin "./target/riscv64gc-unknown-none-elf/debug/initproc"
app_1_end:

    .section .data
    .global app_2_start
    .global app_2_end
    .align 3
app_2_start:
    .incbin "./target/riscv64gc-unknown-none-elf/debug/runtestcase"
app_2_end:

    .section .data
    .global app_3_start
    .global app_3_end
    .align 3
app_3_start:
    .incbin "./target/riscv64gc-unknown-none-elf/debug/shell"
app_3_end:

    .section .data
    .global app_4_start
    .global app_4_end
    .align 3
app_4_start:
    .incbin "./target/riscv64gc-unknown-none-elf/debug/busybox"
app_4_end:
