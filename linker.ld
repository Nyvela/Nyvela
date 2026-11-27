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
    kernel_bss_start = .;
    *(COMMON)
    *(.bss*)
    kernel_bss_end = .;
  }

  .stack : ALIGN(4096)
  {
    kernel_stack_bottom = .;
    . += 0x4000; /* 16 KiB */
    kernel_stack_top = .;
  }

  kernel_end = .;

  /DISCARD/ :
  {
    *(.eh_frame .eh_frame.*)
  }
}
