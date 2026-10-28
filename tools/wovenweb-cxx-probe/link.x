ENTRY(_start)

PHDRS { text PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); tls PT_TLS FLAGS(4); }

SECTIONS
{
    . = 0x400000000000;
    .text : ALIGN(4096) { *(.text.start) *(.text .text.*) *(.ltext .ltext.*) } :text
    .rodata : ALIGN(4096) { *(.rodata .rodata.*) *(.lrodata .lrodata.*) } :text
    . = ALIGN(4096);
    .data : ALIGN(4096) { *(.data .data.*) *(.ldata .ldata.*) *(.got .got.plt) } :data
    .tdata : ALIGN(64) { *(.tdata .tdata.*) } :data :tls
    .tbss : ALIGN(64) { *(.tbss .tbss.*) } :data :tls
    .init_array : ALIGN(8) {
        PROVIDE_HIDDEN(__init_array_start = .);
        KEEP(*(SORT_BY_INIT_PRIORITY(.init_array.*)))
        KEEP(*(.init_array))
        PROVIDE_HIDDEN(__init_array_end = .);
    } :data
    .bss : ALIGN(4096) { *(.bss .bss.*) *(.lbss .lbss.*) *(COMMON) } :data
    /DISCARD/ : { *(.eh_frame .eh_frame_hdr .comment .note .note.*) }
}
