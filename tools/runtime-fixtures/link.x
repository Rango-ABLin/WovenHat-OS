ENTRY(_start)
PHDRS {
  text PT_LOAD FLAGS(5);
  rodata PT_LOAD FLAGS(4);
  data PT_LOAD FLAGS(6);
  tls PT_TLS FLAGS(4);
}
SECTIONS {
  . = 0x400000000000;
  .text : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.text .text.*) } :text
  .rodata : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.rodata .rodata.*) } :rodata
  .data : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.data .data.*) } :data
  .tdata : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.tdata .tdata.*) } :data :tls
  .tbss : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.tbss .tbss.*) } :data :tls
  .bss : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.bss .bss.*) *(COMMON) } :data
}
