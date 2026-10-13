/* WovenHat userspace program layout.
 *
 * The kernel's ELF loader requires a static ET_EXEC image whose entry lies at
 * or above USER_REGION_START (0x4000_0000_0000), rejects PT_DYNAMIC, PT_INTERP,
 * PT_TLS and PT_GNU_RELRO, and refuses any segment that is both writable and
 * executable. Code and data are therefore placed in separate page-aligned
 * segments so W^X holds by construction.
 *
 * The large code model (see .cargo/config.toml) names its sections .ltext,
 * .lrodata, .ldata and .lbss. They must be placed explicitly: an unmatched
 * section is an orphan the linker puts in the first segment, which is R_X,
 * so a writable static there faults on its first write.
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
        *(.ltext .ltext.*)
    } :text

    .rodata : ALIGN(4096) {
        *(.rodata .rodata.*)
        *(.lrodata .lrodata.*)
    } :text

    . = ALIGN(4096);

    .data : ALIGN(4096) {
        *(.data .data.*)
        *(.ldata .ldata.*)
        *(.got .got.plt)
    } :data

    .bss : ALIGN(4096) {
        *(.bss .bss.*)
        *(.lbss .lbss.*)
        *(COMMON)
    } :data

    /DISCARD/ : {
        *(.eh_frame .eh_frame_hdr)
        *(.comment)
        *(.note .note.*)
    }
}
