# Nyvela

A simple 64-bit operating system made from scratch in x86-64 assembly and C.

Nyvela boots off a raw disk image through a three-stage bootloader, walks the CPU
from real mode to long mode, sets up paging, and lands in a freestanding kernel
that brings up memory management, interrupts, a scheduler, a filesystem, and
userspace.

## Where it stands

Working:

- three-stage boot chain, real mode -> protected mode -> long mode, A20, E820
- GDT + TSS, IDT, PIC, LAPIC timer, PS/2 keyboard
- PMM (bitmap), VMM (4-level paging), kernel heap
- processes with per-process address spaces, round-robin scheduler, ring0/ring3
  context switches
- `int $0x80` syscalls: console, ramfs, exec, threads, address spaces, pages, IPC
- a userspace init that spawns a separate ring3 shell

Work-In-Progress: fault handlers log and halt rather than recovering, there's no IST,
no SMP, no persistent filesystem, no ELF loader.

## Quick start

You need `gcc`, `nasm`, `ld`, `objcopy`, `make`, and `qemu-system-x86_64`.

One bit of setup first. This repo is kernel-only, so `make` expects a
`nyvela.conf` pointing at the two userspace programs it boots into: an init
program and a shell. It will not run without one.

Example config:

```make
# nyvela.conf
INIT_DIR = ../Nyvd
INIT_BIN_NAME = nyvd
INIT_BUILD_COMMAND = USER_CFLAGS="-I$(CURDIR)/include" make -C $(INIT_DIR)

SHELL_DIR = ../nyvsh
SHELL_BIN_NAME = shell
SHELL_BUILD_COMMAND = make -C $(SHELL_DIR)
```

Then:

```sh
make        # -> build/os.img
make run    # boot it
make debug  # boot paused, GDB stub on :1234
```

You should land at a `Nyvela >` prompt. Try `help`, `ls`, `cat readme.txt`, and
`run hello`.

## Shape of it

```
BIOS -> boot.s (0x7C00) -> stage_1.s (0x8000) -> stage_2.s (0x8800) -> kmain (0x100000)
```

- **boot.s** loads stage 1 and jumps to it.
- **stage_1.s** loads stage 2 and the kernel, reads the E820 map, enables A20,
  builds the GDT, enters protected mode.
- **stage_2.s** enables PAE and long mode, sets up an identity map, relocates the
  staged kernel to `0x100000`, jumps to `kmain`.
- **kmain** initialises the PMM, VMM, heap, IDT, PIC, LAPIC, keyboard, VFS and
  syscalls, runs its self-tests, then spawns a ring0 idle process and drops into
  ring3. Everything after that is userspace.

## Layout

```
include/nyvela/     headers: arch, mm, thread, process, syscall, fs, user SDK
src/arch/x86_64/    boot loaders, GDT, IDT, context switch, PIC, APIC, faults
src/mm/             pmm.c, vmm.c, heap.c
src/thread/         threads and context switching
src/process/        process table
src/syscall/        the int 0x80 dispatcher
src/fs/             vfs.c, ramfs.c
src/exec/           flat binary loader
src/as/             address spaces
src/ipc/            inter-process messages
src/user/           hello/, ld/ (prog.ld), blobs/
docs/               the actual documentation
```

## Documentation

The `docs/` directory has the detail:

| doc | what's in it |
|-----|--------------|
| [Boot chain](docs/boot.md) | the three loaders, image layout, memory management, interrupts |
| [Build system](docs/build.md) | `nyvela.conf`, toolchains, linker scripts, flat binaries, adding a user program |
| [Syscalls](docs/syscall.md) | the `int 0x80` ABI and full table, validation rules |
| [Userspace](docs/userspace.md) | processes, address spaces, memory layout, init bootstrap, IPC |
| [Debugging](docs/debugging.md) | GDB recipes, capturing the console, troubleshooting table |

## Roadmap

### Boot

- [x] Boot from disk (MBR -> `stage_1` -> `stage_2`)
- [x] Enable A20 through the BIOS
- [x] GDT + TSS, protected mode
- [x] PAE, long mode, paging
- [x] Load the kernel to `0x100000`

### Kernel

- [x] Physical memory management (bitmap PMM)
- [x] Virtual memory management (4-level paging, per-PML4 helpers)
- [x] Kernel heap (`kmalloc` / `krealloc` / `kfree`)
- [x] Interrupts and exceptions (IDT, PIC, LAPIC timer, keyboard)
- [x] Threads and a round-robin scheduler
- [x] Processes and address spaces
- [x] Syscalls (`int $0x80`)
- [ ] Fault recovery - handlers currently log and halt
- [ ] IST stacks so a ring3 fault cannot take the kernel down
- [ ] SMP

### Userspace

- [x] Ring3 execution with `U/S` pages and syscalls
- [x] Per-process address spaces
- [x] User SDK (`syslib.h`)
- [x] Shell (`help` / `ls` / `cat` / `echo` / `run` / `clear` / `exit`)
- [x] VFS + ramfs
- [x] Detach the init program fully from the kernel
- [x] IPC: receive side, and move the scratch address off the target's code page
- [ ] Process reaping, and ownership-checked user pointers
- [ ] ELF loader, so user programs stop needing a flat binary at a fixed base
- [ ] Persistent filesystem

### Graphics

- [x] VGA text-mode console
- [x] PS/2 keyboard input
- [ ] Framebuffer support
- [ ] Graphical interface

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

GPL-3.0. See `LICENSE`.
