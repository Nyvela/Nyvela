# Lumen

A simple 64-bit operating system written in x86-64 assembly, C, and Rust.

Lumen boots off a raw disk image through a three-stage bootloader, walks the CPU
from real mode to long mode, sets up paging, and lands in a freestanding kernel
that brings up memory management, interrupts, a scheduler, a filesystem, and
userspace.

## Where it stands

Working:

- three-stage boot chain, real mode -> protected mode -> long mode, A20, E820
- GDT + TSS, IDT, PIC, LAPIC timer, PS/2 keyboard
- PMM (bitmap), VMM (4-level paging), kernel heap
- threads with per-thread address spaces, round-robin scheduler, ring0/ring3
  context switches
- `int $0x80` syscalls: console, ramfs, exec, threads, address spaces, pages, IPC
- a userspace init that spawns a separate ring3 shell
- FAT32 file and directory operations on an NVMe namespace through Lumfat
- ACPI processor discovery and Rust kernel workers on secondary xAPIC CPUs
- static x86-64 ELF processes with private address spaces and a musl syscall ABI
- Rust `std`, SQLite and DoomGeneric applications
- QEMU Standard VGA graphics and RTL8139 networking with IPv4, TCP and UDP

The boot processor runs the userspace scheduler. Secondary processors run
explicitly dispatched Rust kernel jobs.

## Quick start

You need `gcc` with C23 support, `nasm`, `ld`, `objcopy`, `make`,
`qemu-system-x86_64`, and Rust 1.88 or later. Install the freestanding Rust
target with `rustup target add x86_64-unknown-none`. Install `dosfstools` to
create the default FAT32 disk on the first boot.

Clone the userspace services and Rust drivers alongside the kernel checkout.
`lumen.conf` can override the legacy service paths.

```sh
git clone https://github.com/amogus-gggy/Lumd.git
git clone https://github.com/amogus-gggy/Lumsh.git
git clone https://github.com/amogus-gggy/Lpcid.git
git clone https://github.com/amogus-gggy/nvmed-rs.git
git clone https://github.com/amogus-gggy/Lumfat.git
git clone https://github.com/amogus-gggy/Lumfw.git
git clone https://github.com/amogus-gggy/Lumgfx.git
git clone https://github.com/amogus-gggy/Lumeth.git
```

Example config:

```make
# lumen.conf
INIT_DIR = ../Lumd
INIT_BIN_NAME = lumd
INIT_BUILD_COMMAND = USER_CFLAGS="-I$(CURDIR)/include $(X86_CFLAGS)" $(MAKE) -B -C $(INIT_DIR)

SHELL_DIR = ../Lumsh
SHELL_BIN_NAME = shell
SHELL_BUILD_COMMAND = make -C $(SHELL_DIR)

NVMED_DIR = ../nvmed-rs
NVMED_BIN_NAME = nvmed

LPCID_DIR = ../Lpcid
LPCID_BIN_NAME = lpcid

FAT_DIR = ../Lumfat
```

Then:

```sh
make        # -> build/os.img
make run    # boot it
make debug  # boot paused, GDB stub on :1234
```

You should land at a `Lumen >` prompt. Try `help`, `ls`, `cat readme.txt`, and
`run hello`.

`make run` starts four virtual CPUs and attaches `build/fat32.img` through
NVMe. The terminal shows the CPU count and `NVMED: ready` once the driver starts.
At the Lumen prompt in the QEMU window, type `run fat`. Inside `fat`, use
`ls`, `cat <path>`, `mkdir <path>`, `write <path> <text>`, and `exit`.
Changes persist in `build/fat32.img`.

Build and boot the optional ELF application archive with `make run-ports`.
See [Static ELF applications](docs/ports.md) for dependencies, examples and
toolchain commands.

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
  syscalls, runs its self-tests, then spawns a ring0 idle thread and drops into
  ring3. Everything after that is userspace.

## Layout

```
include/lumen/     headers: arch, mm, thread, syscall, fs, user SDK
src/arch/x86_64/    boot loaders, GDT, IDT, context switch, PIC, APIC, faults
src/mm/             pmm.c, vmm.c, heap.c
src/thread/         threads and context switching
src/syscall/        the int 0x80 dispatcher
src/fs/             vfs.c, ramfs.c
src/exec/           legacy flat binary loader
src/kernel/rust/user/ ELF process loader, syscall ABI and device integration
ports/              ELF applications and build rules
src/as/             address spaces
src/ipc/            inter-thread messages
src/user/           hello/, ld/ (prog.ld), blobs/
docs/               the actual documentation
```

## Documentation

The `docs/` directory has the detail:

| doc | what's in it |
|-----|--------------|
| [Boot chain](docs/boot.md) | the three loaders, image layout, memory management, interrupts |
| [Build system](docs/build.md) | `lumen.conf`, toolchains, linker scripts, flat binaries, adding a user program |
| [Syscalls](docs/syscall.md) | the `int 0x80` ABI and full table, validation rules |
| [Userspace](docs/userspace.md) | threads, address spaces, memory layout, init bootstrap, IPC |
| [Static ELF applications](docs/ports.md) | Rust and Clang toolchains, SQLite, Doom and IPv4 networking |
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
- [x] Address spaces (`as_create` / `as_map` / `as_free`)
- [x] Syscalls (`int $0x80`)
- [x] Terminate and reap faulting foreground ELF processes
- [x] IST stacks for page faults and double faults
- [x] Start secondary xAPIC processors and run Rust kernel jobs
- [ ] Schedule userspace threads on secondary processors

### Userspace

- [x] Ring3 execution with `U/S` pages and syscalls
- [x] Per-thread address spaces
- [x] User SDK (`syslib.h`)
- [x] Shell (`help` / `ls` / `cat` / `echo` / `run` / `clear` / `exit`)
- [x] VFS + ramfs
- [x] FAT32 file and directory operations over NVMe IPC
- [x] Detach the init program fully from the kernel
- [x] IPC: receive side, and move the scratch address off the target's code page
- [x] ELF foreground process reaping and page-checked syscall buffers
- [x] Static x86-64 ELF loader

### Graphics

- [x] VGA text-mode console
- [x] PS/2 keyboard input
- [x] QEMU Standard VGA linear framebuffer
- [ ] Graphical interface

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

GPL-3.0. See `LICENSE`.
