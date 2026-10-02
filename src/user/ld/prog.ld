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
