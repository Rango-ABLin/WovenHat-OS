ENTRY(_start)

PHDRS { text PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); }

SECTIONS
{
    . = 0x400000000000;
    .text : ALIGN(4096) { *(.text.start) *(.text .text.*) *(.ltext .ltext.*) } :text
    .rodata : ALIGN(4096) { *(.rodata .rodata.*) *(.lrodata .lrodata.*) } :text
    . = ALIGN(4096);
    .data : ALIGN(4096) { *(.data .data.*) *(.ldata .ldata.*) *(.got .got.plt) } :data
    .bss : ALIGN(4096) { *(.bss .bss.*) *(.lbss .lbss.*) *(COMMON) } :data
    /DISCARD/ : { *(.eh_frame .eh_frame_hdr .comment .note .note.*) }
}
