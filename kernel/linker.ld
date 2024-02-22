OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{
    .text : ALIGN(4K){
        _stext = .;
        *(.text.entry)
        . = ALIGN(4K);
        *(.text .text.*)
        _etext = .;
    } > VIRT AT> RAM

    .rodata : ALIGN(4K){
        _srodata = .;
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
        _erodata = .;
    } > VIRT AT> RAM

    .data : ALIGN(4K){
        _sdata = .;
        *(.data .data.*)
        *(.sdata .sdata.*)
        _edata = .;
    } > VIRT AT> RAM

    .bss : ALIGN(4K){
        _sstack = .;
        *(.bss.stack)
        _estack = .;
        _sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        _ebss = .;
    } > VIRT AT> RAM

    /DISCARD/ : {
        *(.eh_frame)
    }
}

MEMORY
{
    VIRT : ORIGIN = %VIRT_START%, LENGTH = %RAM_SIZE%
    RAM : ORIGIN = %RAM_START%, LENGTH = %RAM_SIZE%
}
