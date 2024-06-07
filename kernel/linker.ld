OUTPUT_ARCH(riscv)
ENTRY(_start)
BASE_ADDRESS = 0xffffffc080200000;

SECTIONS
{
    . = BASE_ADDRESS;
    _skernel = .;

    _stext = .;
    .text : {
        *(.text.entry)
        . = ALIGN(4K);
        _strampoline = .;
        *(.text.trampoline)
        . = ALIGN(4K);
        _etrampoline = .;
        *(.text .text.*)
    }

    . = ALIGN(4K);
    _etext = .;
    _srodata = .;
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    . = ALIGN(4K);
    _erodata = .;
    _sdata = .;
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    . = ALIGN(4K);
    _edata = .;
    _sstack = .;
    /* boot stack(in entry.S) */
    .stack : {
        *(.bss.stack)
    }

    . = ALIGN(4K);
    _estack = .;
    /* sbss_with_stack = .; */
    _sbss = .;
    .bss : {
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }

    . = ALIGN(4K);
    _ebss = .;
    _ekernel = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}
