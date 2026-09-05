ENTRY(kmain)

SECTIONS
{
  . = 0x100000;

  .text : ALIGN(4096)
  {
    *(.text.entry)
    *(.text*)
  }

  .rodata : ALIGN(4096)
  {
    *(.rodata*)
  }

  .data : ALIGN(4096)
  {
    *(.data*)
  }

  .bss : ALIGN(4096)
  {
    *(COMMON)
    *(.bss*)
  }
}
