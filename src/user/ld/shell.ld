/* Flat user binary linked for SHELL_BASE (0x400000).
 * _start lands exactly at BASE: USER_CFLAGS uses -ffunction-sections and
 * .text._start is kept first. No libc, no relocations: the kernel copies
 * this blob verbatim to its linked address with U/S pages.
 */
ENTRY(_start)

SECTIONS
{
  . = 0x400000;

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
