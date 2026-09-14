/* Flat user binary linked for PROG_BASE (0x500000). See shell.ld. */
ENTRY(_start)

SECTIONS
{
  . = 0x500000;

  .text : {
    *(.text._start)
    *(.text*)
  }

  .rodata : {
    *(.rodata*)
  }

  .data : {
    *(.data*)
  }

  /DISCARD/ : {
    *(.eh_frame*)
    *(.comment*)
    *(.note*)
  }

  .bss : {
    *(.bss*)
    *(COMMON)
  }
}
