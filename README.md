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
* Identity mapping for low memory (2 MiB pages at boot, converted to 4 KiB pages by `kvmm_init`), PAE and LME/LMA setup
* Physical Memory Manager (bitmap allocator)
* Virtual Memory Manager (4-level paging: PML4, PDPT, PD, PT; `U/S` propagated for user pages)
* Kernel heap allocator (`kmalloc`/`krealloc`/`kfree` with block splitting; single allocation < ~4 KiB)
* VGA text mode console and kernel logging (`kprintinfo`, `kprintsucc`, `kprintfail`, `kprintferr`, `kwrite` for sized buffers)
* IDT with 256 slots, 7 vectors installed (DE/PF/GP stubs, PIT, keyboard `0x21`, LAPIC timer `0x30`, syscall `0x80` DPL3) and LAPIC support (local APIC enable, timer setup, EOI)
* PS/2 keyboard driver (scancode set 1, US layout, IRQ1, 256-byte ring buffer; `kpic_enable_irq`)
* Basic CPU abstraction (CPUID, MSR, port I/O)
* Thread abstraction and round-robin scheduler (per-tick context reuse, no leak; `TSS.rsp0` follows the next thread; exit codes)
* Context switching via `save_context` and `switch_context` (x86-64 `iretq` based, ring0 + ring3 paths)
* TSS with ring0 `rsp0` for ring3 -> ring0 transitions (no IST yet)
* Fault diagnostics (`#UD`/`#DF` dump vector, RIP/CS/RSP + 16 bytes at RIP in hex, then halt)
* Syscalls via `int $0x80` (`exit/write/yield/read/fs_create/fs_write/fs_read/fs_list/exec`; user pointers validated to `0x400000-0x700000`, buffers capped at 2 KiB)
* Minimal ring3 userspace: shared user PML4, shell at `0x400000` + stack `0x450000`, foreground program slot at `0x500000` + stack `0x600000` (`U/S` pages, per-thread kernel stacks)
* Ring3 shell (`help/ls/cat/echo/run/clear/exit`) with line editing; `run` executes `/bin` binaries foreground and prints the exit code
* User SDK (`include/nyvela/user/syslib.h`): freestanding syscall wrappers + string helpers; programs link as flat binaries for fixed bases (`src/user/ld/*.ld`), embedded into the kernel image and published as `/bin/*`
* VFS with single ramfs mount at `/` (absolute paths, files up to 32 KiB in 8 pages, `create/read/write/list/size`)
* Self-tests for PMM, VMM, heap, VFS, and syscalls at boot

In progress:

* Full exception recovery for page faults and general protection faults (current DE/PF/GP handlers only log and halt)
* IST stacks for fault isolation
* SMP support
* Standard library and advanced shell features (pipes, background jobs, arguments)
* Persistent/block-device filesystems and ELF loader (current exec runs flat binaries from ramfs)
* Framebuffer support and graphics

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
3. Run `ktest_palloc`, `ktest_vmmap`, `ktest_kmalloc`, `ktest_krealloc`
4. `kidt_init` (incl. syscall gate `0x80` DPL3 + keyboard `0x21`), `kpic_init`, `kpit_set_freq(1000)`, `kenable_lapic`, `ksetup_lapic_timer`, `kpic_disable` (PIT was only needed to calibrate the LAPIC)
5. `ktss_init`, `kbd_init` (drains 8042, unmasks IRQ1), `vfs_init` (ramfs at `/`), `syscall_init`
6. Run `ktest_vfs`, `ktest_syscall` (yield + bad-fd via `int $0x80` from ring0), publish embedded binaries (`kload_user_bins`: `/bin/hello`, `/readme.txt`)
7. Spawn ring0 idle thread (`hlt` loop), clone user PML4, map shell reservation (`0x400000`, 4 `U/S` pages, zeroed, blob copied) + shell stack (`0x44F000`), map program slot (`0x500000`, 16 zeroed `U/S` pages) + program stack (`0x5FF000`), spawn shell thread with `cs=0x1B/ss=0x23`, set `TSS.rsp0` to its kernel stack, `switch_context` to ring3 (never returns; preemption continues via LAPIC timer interrupt `0x30` at ~1 kHz)

## Syscalls (`int $0x80`)

`rax` = number, args = `rdi, rsi, rdx, r10`, return in `rax` (bytes on success, negative `VFS_ERR_*` cast to `u64` on error).

| nr | name | args | effect |
|----|------|------|--------|
| 0 | `exit` | `code=rdi` | records exit code, marks current thread `DEAD`, switches to next `READY` (never returns) |
| 1 | `write` | `fd` (1/2 = console), `buf` (user), `len` (<=2048) | `kwrite` to VGA console (`\n` + `\b` handled) |
| 2 | `yield` | - | no-op stub, returns 0 (preemption is timer-driven) |
| 3 | `read` | `fd` (0 = keyboard), `buf` (user), `len` (1..2048) | blocking: waits with `sti/hlt` until >=1 byte, returns bytes read |
| 10 | `fs_create` | `path` (user string), `is_dir` | `vfs_create` |
| 11 | `fs_write` | `path`, `buf`, `len`, `offset=r10` | `vfs_write` |
| 12 | `fs_read` | `path`, `buf`, `len`, `offset=r10` | `vfs_read` |
| 13 | `fs_list` | `path`, `buf`, `len` | newline-separated names + NUL, returns bytes excl. NUL |
| 20 | `exec` | `path` (user string) | loads ramfs file into the `0x500000` slot (<=64 KiB), runs it foreground in ring3 sharing the user PML4, blocks until exit, returns its exit code |

User pointers must lie in `0x400000-0x700000` and be `NUL`-terminated within 256 bytes (paths) / 2048 bytes (buffers). Fast syscalls run with `IF=0` (interrupt gate, atomic vs the timer); blocking ones (`read`, `exec`) wait with `sti/hlt` so the timer keeps preempting — safe because every thread has its own kernel stack and `TSS.rsp0` follows the scheduled thread.

## Userspace

* Address space: `vmm_create_user_pml4()` clones the kernel PML4 once at boot; shell, programs, and idle share it. `kvmmap` with flags `0x07` maps user pages and sets `U/S` on the path tables.
* Layout (`U/S` pages): shell image `0x400000-0x404000` (4-page reservation, blob copied over zeroed pages so `.bss` works), shell stack `0x44F000-0x450000`; program slot `0x500000-0x510000` (16 zeroed pages), program stack `0x5FF000-0x600000`.
* Flat binaries: user programs are freestanding (`-nostdlib`, syscalls only), linked for a fixed base (`src/user/ld/shell.ld` -> `0x400000`, `src/user/ld/prog.ld` -> `0x500000`, `_start` first via `-ffunction-sections`), converted with `objcopy -O binary`, embedded into the kernel via `incbin` blobs (`src/user/blobs/`), and at boot copied to their linked addresses (`shell`) or published as ramfs files (`/bin/hello`). The shell itself is Rust (`src/user/shell/`, `no_std`, cargo `x86_64-unknown-none` target); `hello` is C — both toolchains are supported, see `makefile` USER rules.
* Limits (v1): single shared address space (no isolation between shell and programs), one foreground program at a time, no CLI arguments, no thread reaping (`DEAD` exec threads linger, skipped by the scheduler).

## Shell

Ring3 program (`src/user/shell/shell.c`, prompt `nyvela> `) with line editing (echo + backspace):

| command | effect |
|---------|--------|
| `help` | this list |
| `ls [path]` | list directory (default `/`; relative paths ok, e.g. `ls bin`) |
| `cat <file>` | print file (relative ok, e.g. `cat readme.txt`) |
| `echo <...>` | print args |
| `run <name>` | run `/bin/<name>` foreground, print `exit: <code>`; absolute `/bin/` paths accepted, anything outside `/bin/` is refused |
| `clear` | clear screen |
| `exit` | exit shell (system idles) |

Paths are resolved against `/` (trailing slashes stripped) and errors are printed readable (`no such file or directory`, `is a directory`, ...), e.g. `run test` -> `run: /bin/test: no such file or directory`.

## Your own binary

1. Copy `src/user/hello/hello.c` to `src/user/<name>/<name>.c`. Write freestanding C: only `syslib.h` (`sys_*`, `ustr*`, `uitoa`), entry `void _start(void)`, finish with `sys_exit(code)`. No libc, no globals needing nonzero init beyond what the zeroed slot provides (`.bss` starts zero). **Mandatory: `-mno-red-zone`** (already in `USER_CFLAGS`) — `switch_context` pushes the iretq frame below the saved user RSP, which would clobber red-zone locals (same for Rust: `-C no-redzone=yes`).
2. Add build rules in `makefile` (copy the `USER_HELLO_*` block, link with `src/user/ld/prog.ld`), plus a blob file `src/user/blobs/<name>_blob.s` (copy `hello_blob.s`, point `incbin` at your `.bin`).
3. Publish it in `kload_user_bins()` (`src/kernel/kernel.c`): `vfs_create` + `vfs_write` the blob, e.g. as `/bin/<name>`.
4. `make && make run`, then in the shell: `run <name>`.
5. Keep the flat `.bin` under 64 KiB (`PROG_MAX`); the kernel image itself must stay under 96 sectors (`make` fails loudly otherwise).

## Filesystem (VFS + ramfs)

* `include/nyvela/fs/vfs.h` / `src/fs/vfs.c`: absolute-path validator over a single ramfs mount at `/` (future filesystems plug in here).
* `include/nyvela/fs/ramfs.h` / `src/fs/ramfs.c`: in-memory tree (`name[64]`, `child/sibling` links), file data in up to 8 `kpalloc` pages (32 KiB max), node structs via `kmalloc`.
* API: `vfs_init/create/read/write/list/size` (see `vfs.h`); errors are negative `VFS_ERR_*` (`INVAL/NOTFOUND/EXISTS/NOTDIR/ISDIR/NOSPACE/BADPATH`).
* Absolute paths only, no `.`/`..`/symlinks; `write` requires `offset <= size` (no sparse holes).

## Memory Management

* **PMM** (`src/mm/pmm.c`): E820-driven bitmap at `0x10000`. `BITMAP_RESERVED` vs `BITMAP_FREE`, `kernel_end` to end of RAM is allocatable. `kpalloc`/`kpfree` are page-granular (4096 byte).
* **VMM** (`src/mm/vmm.c`): Converts the boot 2 MiB identity map to 4 KiB pages at runtime, supports `kvmmap`/`kvmunmap` for 4 KiB pages (sets/propagates `U/S` when `flags & 0x04`). Uses CR3-reload TLB flush. `vmm_create_user_pml4()` clones the kernel PML4 for user address spaces.
* **Heap** (`src/mm/heap.c`): Single-page bootstrap via `kpalloc`, block header `block_t` with `size`, `is_used`, `next`. `kmalloc` aligns to 16 bytes, splits blocks, allocates new pages on demand. One allocation must be < ~4 KiB; `kfree` marks free without coalescing (freed blocks are reused on later allocs).

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
│   ├── drivers/          # video/console, kbd
│   ├── fs/               # vfs, ramfs
│   ├── lib/              # utils (memcpy, memset, strings, ki64toa)
│   ├── mm/               # pmm, heap, vmm
│   ├── scheduler/        # scheduler
│   ├── syscall/          # syscall ABI + frame
│   ├── thread/           # thread
│   └── user/             # syslib.h (user SDK)
├── src/
│   ├── arch/x86_64/
│   │   ├── apic/         # apic.c
│   │   ├── asm/          # msr, cpuid, port I/O
│   │   ├── boot/         # boot.s, stage_1.s, stage_2.s, idt.s, print helpers
│   │   └── cpu/          # idt.c, isr.s, context.s, gdt.c, pic.c, pit.c
│   ├── drivers/          # video/console+vga, kbd
│   ├── fs/               # vfs.c, ramfs.c
│   ├── kernel/           # kernel.c
│   ├── lib/              # utils
│   ├── mm/               # pmm, heap, vmm
│   ├── scheduler/        # scheduler.c
│   ├── syscall/          # syscall.c
│   ├── thread/           # thread.c
│   └── user/             # shell/, hello/, ld/, blobs/ (flat user binaries)
├── linker.ld
├── makefile
└── README.md
```

Boot image layout: `boot` (1 sector) + `stage_1` (4 sectors) + `stage_2` (32 sectors) + kernel (reserved 96 sectors at LBA 37, `DAP_kernel`/`KERNEL_SIZE_IN_SECTORS`). `make` pads `os.img` to 133 sectors so the BIOS read never runs past EOF, and fails loudly if `kernel.bin` outgrows 96 sectors.

## Roadmap

### Boot

* [x] Boot from disk
* [x] Enable A20
* [x] Set up GDT and IDT
* [x] Enable protected mode
* [x] Enable long mode and paging
* [x] Load kernel to `0x100000`

### Kernel

* [x] Initialize hardware (LAPIC + LAPIC timer)
* [x] Implement physical memory management (`kpmm_init`, `kpalloc`/`kpfree`)
* [x] Implement virtual memory (`kvmm_init`, `kvmmap`/`kvmunmap`)
* [x] Implement interrupts and exceptions (IDT + DE/PF/GP stubs + PIT `0x20` + keyboard `0x21` + LAPIC timer tick `0x30`)
* [x] Implement a heap allocator (`kmalloc`/`krealloc`/`kfree`)
* [x] Implement basic threads (`spawn_thread` + exit codes; no processes yet)
* [x] Implement a basic scheduler (round-robin on LAPIC timer; per-tick context reuse, `TSS.rsp0` follows next thread)
* [x] Implement system calls (`int $0x80`: exit/write/yield/read/fs_create/fs_write/fs_read/fs_list/exec)
* [x] Add x86-64 asm abstractions (`arch/x86_64/asm/`: cpu/io/desc/msr/cpuid)
* [ ] Stable preemptive scheduler under load (works at ~1 kHz; SMP/load stress not done)
* [ ] Full exception handling (DE, PF, GP recovery; handlers currently log and halt)
* [ ] SMP support
* [x] TSS for fault isolation (ring0 `rsp0` only for now)
* [ ] IST stacks

### Userspace

* [x] Minimal userspace support and privilege separation (ring3 shell + foreground programs, `U/S` pages, syscalls; single shared address space, no ELF loader yet)
* [ ] Standard library
* [x] Shell and basic utilities (`help/ls/cat/echo/run/clear/exit`; no pipes/jobs/args yet)
* [x] Filesystem support (minimal VFS + ramfs at `/`; no persistent/block-device FS yet)

### Graphics

* [ ] Implement framebuffer support
* [x] Implement text rendering (VGA text-mode console; no framebuffer fonts yet)
* [x] Implement input handling (minimal PS/2 keyboard: set 1, US, IRQ1; no arrows/numpad/combos yet)
* [ ] Build a graphical interface

## Contributing

Contributions are welcome. Please open an issue or pull request, keep commits focused, add tests where applicable, and ensure `make` builds cleanly.

## License

GPL-3.0. See `LICENSE` for details.
