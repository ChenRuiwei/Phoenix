OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{
    . = ALIGN(4K);
    .text : {
        *(.text.entry)
        . = ALIGN(4K);
        *(.text .text.*)
    } > VIRT AT> RAM

    . = ALIGN(4K);
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    } > VIRT AT> RAM

    . = ALIGN(4K);
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    } > VIRT AT> RAM

    . = ALIGN(4K);
    .bss : {
        *(.bss.stack)
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    } > VIRT AT> RAM

    . = ALIGN(4K);

    /DISCARD/ : {
        *(.eh_frame)
    }
}

MEMORY
{
    VIRT : ORIGIN = %VIRT_START%, LENGTH = %RAM_SIZE%
    RAM : ORIGIN = %RAM_START%, LENGTH = %RAM_SIZE%
}
