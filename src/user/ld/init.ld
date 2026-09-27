/* Flat user binary for the init process, linked for USER_CODE_VIRT (0x400000).
 * _start lands exactly at BASE: USER_CFLAGS uses -ffunction-sections and
 * .text._start is kept first. No libc, no relocations: the kernel loads
 * this blob from ramfs /bin/init into its linked address with U/S pages.
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
