OUTPUT_FORMAT(elf64-x86-64)
ENTRY(kmain)

SECTIONS
{
    . = 0x200000;

    .text : {
        KEEP(*(.text.magic))
        KEEP(*(.text.entry))
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    }
    PROVIDE(__kernel_end = .);

    /DISCARD/ : {
        *(.eh_frame*)
        *(.note .note.*)
    }
}
