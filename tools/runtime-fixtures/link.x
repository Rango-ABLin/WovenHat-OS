ENTRY(_start)
PHDRS {
  text PT_LOAD FLAGS(5);
  rodata PT_LOAD FLAGS(4);
  data PT_LOAD FLAGS(6);
  tls PT_TLS FLAGS(6);
}
SECTIONS {
  . = 0x400000000000;
  .text : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.text .text.*) } :text
  .rodata : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.rodata .rodata.*) } :rodata
  . = ALIGN(CONSTANT(MAXPAGESIZE));
  .data : { *(.data .data.*) } :data
  .tdata : ALIGN(8) { *(.tdata .tdata.*) } :tls :data
  .tbss : ALIGN(8) { *(.tbss .tbss.*) } :tls :data
  .bss : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.bss .bss.*) *(COMMON) } :data
}
