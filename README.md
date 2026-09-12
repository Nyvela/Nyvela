# Nyvela

A modern 64-bit operating system written from scratch in x86-64 Assembly and C.

Nyvela is an experimental operating system focused on understanding and implementing the fundamentals of a computer system, from the boot process and CPU initialization to memory management, hardware support, and userspace.

## Overview

Nyvela boots from a raw disk image via a custom three-stage bootloader, transitions the CPU from real mode to protected mode to long mode, sets up paging, and jumps to a freestanding C kernel. The kernel is loaded at `0x100000`, uses a higher-half capable page table setup, and runs with no standard library, no red zone, and no position independent code.

The project pursues a microkernel-inspired design where the privileged kernel stays minimal and drivers, filesystems, and services are intended to move to userspace as the system matures.

## Features

Implemented:

* Custom boot chain: `boot` (MBR) -> `stage_1` -> `stage_2`
* Real mode to protected mode to long mode transition
* A20 gate enable via BIOS
* E820 memory map parsing
* GDT with null, kernel code, kernel data, user code, user data, and 64-bit code segments
* Identity mapping for low memory with 2 MiB pages, PAE and LME/LMA setup
* Physical Memory Manager (bitmap allocator)
* Virtual Memory Manager (4-level paging: PML4, PDPT, PD, PT)
* Kernel heap allocator (`kmalloc`/`kfree` with block splitting)
* VGA text mode console and kernel logging (`kprintinfo`, `kprintsucc`, `kprintfail`, `kprintferr`)
* IDT with 256 entries and LAPIC support (local APIC enable, timer setup, EOI)
* Basic CPU abstraction (CPUID, MSR, port I/O)
* Thread abstraction and round-robin scheduler
* Context switching via `save_context` and `switch_context` (x86-64 `iretq` based)
* Self-tests for PMM, VMM, and heap at boot

In progress:

* Preemptive scheduling stability
* Exception handling for page faults and general protection faults
* System calls and userspace isolation
* Filesystems, standard library, shell, and graphics

## Architecture

```
BIOS -> boot.s (0x7C00) -> stage_1.s (0x8000) -> stage_2.s (0x8800) -> kernel @ 0x100000 (kmain)
```

* **boot.s**: MBR, sets up segments, loads `stage_1` via INT 13h extensions, far jump to `0x0800:0x0000`.
* **stage_1.s**: Loads `stage_2` and kernel, collects E820 map at `0xD000`, enables A20, builds GDT, enters protected mode.
* **stage_2.s**: Disables paging, enables PAE, sets LME in EFER, builds PML4/PDPT/PD at fixed tables, enables paging, far jump to `0x28:long_mode_enter`, copies kernel from `0x10000` to `0x100000`, jumps to `kmain`.

Kernel execution (`src/kernel/kernel.c:kmain`):

1. Log RAM map and usable RAM
2. `kpmm_init` -> `kvmm_init` -> `kmalloc_init`
3. Run `ktest_palloc`, `ktest_vmmap`, `ktest_kmalloc`
4. `kidt_init`, `kenable_lapic`, `ksetup_lapic_timer`
5. `sti` and `hlt` loop (scheduler takes over via LAPIC timer interrupt `0x20`)

## Memory Management

* **PMM** (`src/mm/pmm.c`): E820-driven bitmap at `0x10000`. `BITMAP_RESERVED` vs `BITMAP_FREE`, `kernel_end` to end of RAM is allocatable. `kpalloc`/`kpfree` are page-granular (4096 byte).
* **VMM** (`src/mm/vmm.c`): Creates PML4 at runtime, maps low memory with large pages, supports `kvmmap`/`kvmunmap` for 4 KiB pages. Uses `invlpg` style CR3 reload.
* **Heap** (`src/mm/heap.c`): Single-page bootstrap via `kpalloc`, block header `block_t` with `size`, `is_used`, `next`. `kmalloc` aligns to 16 bytes, splits blocks, allocates new pages on demand.

Linker script (`linker.ld`) places `.text.entry` (containing `kmain`) first at `0x100000`, followed by `.rodata`, `.data`, `.bss`, and exports `kernel_end`.

## Requirements

* `gcc` with x86-64 support and freestanding flags
* `nasm` for `elf64` objects and flat binaries
* `ld` and `objcopy`
* `qemu-system-x86_64` for running and debugging
* `make`

Tested on Linux with GCC 13+ and QEMU 8+.

## Building

```sh
make        # builds build/boot.bin, stage_1.bin, stage_2.bin, kernel.elf, kernel.bin, os.img
make clean  # removes build/
```

Outputs:

* `build/kernel.elf` - ELF with symbols for debugging
* `build/kernel.bin` - flat binary
* `build/os.img` - concatenated boot image (`boot + stage_1 + stage_2 + kernel`)

## Running

```sh
make run
```

Runs:

```sh
qemu-system-x86_64 -drive format=raw,file=build/os.img -d int,cpu_reset,guest_errors
```

## Debugging

```sh
make debug
```

Runs QEMU with:

```sh
qemu-system-x86_64 -drive format=raw,file=build/os.img -d int,cpu_reset,guest_errors -D qemu.log -no-reboot -no-shutdown -s -S
```

Then in another terminal:

```sh
gdb build/kernel.elf
(gdb) target remote :1234
(gdb) break kmain
(gdb) continue
```

`qemu.log` contains CPU resets and interrupt traces.

## Project Structure

```
.
├── include/nyvela/
│   ├── arch/x86_64/      # GDT, IDT, context, APIC, MSR, CPUID, port I/O
│   ├── drivers/video/    # VGA and console
│   ├── lib/              # utils (memcpy, memset, ki64toa)
│   ├── mm/               # pmm, heap, vmm
│   ├── scheduler/        # scheduler
│   └── thread/           # thread
├── src/
│   ├── arch/x86_64/
│   │   ├── apic/         # apic.c
│   │   ├── asm/          # msr, cpuid, port I/O
│   │   ├── boot/         # boot.s, stage_1.s, stage_2.s, idt.s, print helpers
│   │   └── cpu/          # idt.c, isr.s, context.s
│   ├── drivers/video/    # console, vga
│   ├── kernel/           # kernel.c
│   ├── lib/              # utils
│   ├── mm/               # pmm, heap, vmm
│   ├── scheduler/        # scheduler.c
│   └── thread/           # thread.c
├── linker.ld
├── makefile
└── README.md
```

## Roadmap

### Boot

* [x] Boot from disk
* [x] Enable A20
* [x] Set up GDT and IDT
* [x] Enable protected mode
* [x] Enable long mode and paging
* [x] Load kernel to `0x100000`

### Kernel

* [x] Physical memory management
* [x] Virtual memory management
* [x] Heap allocator
* [x] LAPIC and timer interrupts
* [x] Threads and context switching
* [ ] Stable preemptive scheduler
* [ ] Full exception handling (DE, PF, GP)
* [ ] System calls
* [ ] SMP support
* [ ] TSS and IST for fault isolation (ring 0 only for now)

### Userspace

* [ ] Userspace support and privilege separation
* [ ] Standard library
* [ ] Shell and basic utilities
* [ ] Filesystem support

### Graphics

* [ ] Framebuffer support
* [ ] Text rendering
* [ ] Input handling
* [ ] Graphical interface

## Contributing

Contributions are welcome. Please open an issue or pull request, keep commits focused, add tests where applicable, and ensure `make` builds cleanly.

## License

GPL-3.0. See `LICENSE` for details.
