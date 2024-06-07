OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{
    .text : ALIGN(4K){
        _skernel = .;
        _stext = .;
        *(.text.entry)
        . = ALIGN(4K);
        /* the reason why sigreturn trampoline need a Page is that this Page has U flag but .text Page doesn't */
        _strampoline = .;
        *(.text.trampoline)
        . = ALIGN(4K);
        _etrampoline = .;
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

    .stack : ALIGN(4K){
        _sstack = .;
        *(.bss.stack)
        . = ALIGN(4K);
        _estack = .;
    } > VIRT AT> RAM

    .bss : ALIGN(4K){
        _sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        _ebss = .;
    } > VIRT AT> RAM

    . = ALIGN(4K);
    _ekernel = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}

MEMORY
{
    VIRT : ORIGIN = %VIRT_START%, LENGTH = %RAM_SIZE%
    RAM : ORIGIN = %RAM_START%, LENGTH = %RAM_SIZE%
}
