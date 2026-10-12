/* WovenHat userspace program layout.
 *
 * The kernel's ELF loader requires a static ET_EXEC image whose entry lies at
 * or above USER_REGION_START (0x4000_0000_0000), rejects PT_DYNAMIC, PT_INTERP,
 * PT_TLS and PT_GNU_RELRO, and refuses any segment that is both writable and
 * executable. Code and data are therefore placed in separate page-aligned
 * segments so W^X holds by construction.
 *
 * The region above 0x4000_0000_0e_0000 belongs to the mmap arena and the stack,
 * so an image must stay below it.
 */
ENTRY(_start)

PHDRS
{
    text PT_LOAD FLAGS(5); /* R_X */
    data PT_LOAD FLAGS(6); /* RW_ */
}

SECTIONS
{
    . = 0x400000000000;

    .text : ALIGN(4096) {
        *(.text.start)
        *(.text .text.*)
    } :text

    .rodata : ALIGN(4096) {
        *(.rodata .rodata.*)
    } :text

    . = ALIGN(4096);

    .data : ALIGN(4096) {
        *(.data .data.*)
        *(.got .got.plt)
    } :data

    .bss : ALIGN(4096) {
        *(.bss .bss.*)
        *(COMMON)
    } :data

    /DISCARD/ : {
        *(.eh_frame .eh_frame_hdr)
        *(.comment)
        *(.note .note.*)
    }
}
