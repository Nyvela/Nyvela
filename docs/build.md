# Build system

## Requirements

* `gcc` with x86-64 and C23 support (GCC 13+)
* `nasm`
* `ld`, `objcopy`
* `make`
* `qemu-system-x86_64`
* `dosfstools` to create the default FAT32 disk image
* Rust 1.88+ with the `x86_64-unknown-none` target

Tested on Linux with GCC 13+ and QEMU 8+.

## lumen.conf

The legacy service programs live in sibling repositories. `make` defaults to
the standard checkout names and accepts overrides in `lumen.conf`.
Clone Lumfw, Lumgfx, Lumeth and Lumouse alongside Lumen for the kernel's Rust drivers.
For the Clang and Rust standard-library toolchains, see
[Static ELF applications](ports.md).

```make
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

| key | meaning |
|-----|---------|
| `INIT_DIR` | directory of the init project - must exist, `make` errors out otherwise |
| `INIT_BIN_NAME` | base name of its flat binary; `make` expects `$(INIT_DIR)/build/$(INIT_BIN_NAME).bin` |
| `INIT_BUILD_COMMAND` | command that rebuilds it; runs before every kernel object so blob edits are picked up |
| `SHELL_DIR` | directory of the shell project |
| `SHELL_BIN_NAME` | base name of its flat binary; `make` expects `$(SHELL_DIR)/build/$(SHELL_BIN_NAME).bin` |
| `SHELL_BUILD_COMMAND` | command that rebuilds it |
| `NVMED_DIR` | directory of the `nvmed-rs` project (Rust NVMe driver) |
| `NVMED_BIN_NAME` | base name of its flat binary; published as `/bin/nvmed` |
| `LPCID_DIR` | directory of the `lpcid` project |
| `LPCID_BIN_NAME` | base name of its flat binary; published as `/bin/lpcid` |
| `FAT_DIR` | directory of the Lumfat checkout, which produces `build/fat.bin` |

Lumen invokes recursive builds for nvmed, lpcid, and Lumfat.

Lumen embeds the userspace binaries and publishes them into ramfs at boot.
The assembly blobs use `incbin` and Lumfat uses Rust's `include_bytes!`.

| blob | published as |
|------|--------------|
| `init_blob.s` | `/bin/init` |
| `shell_blob.s` | `/bin/sh` |
| `nvmed_blob.s` | `/bin/nvmed` |
| `lpcid_blob.s` | `/bin/lpcid` |
| `exec.rs` | `/bin/fat` |

Each userspace project provides its own flat binary.

`INIT_BUILD_COMMAND` must forward `USER_CFLAGS`, if the init project includes
Lumen headers (`<lumen/user/syslib.h>`) and only learns the include path from
this invocation. Building it by hand without that flag fails with
`fatal error: lumen/user/syslib.h: No such file or directory`.

### How staleness is tracked

The init binary is a prerequisite of every kernel object, but on its own that is
not enough: `make` only reruns `$(INIT_BUILD_COMMAND)` when a listed prerequisite
is newer than the target. Depending on `lumen.conf` alone means editing init
sources does nothing, `lumd.bin` keeps its old timestamp, and `incbin` embeds
the previous build. The symptom is a kernel that boots a feature you already
wrote - a new spawn target, a new `/bin` entry - and silently keeps the old
behaviour, with no error anywhere.

So the rule also depends on the init project's own sources:

```make
INIT_INPUTS := $(shell find $(INIT_DIR)/src $(INIT_DIR)/include -type f 2>/dev/null) \
               $(INIT_DIR)/makefile $(INIT_DIR)/linker.ld

$(INIT_BIN): makefile $(wildcard lumen.conf) $(INIT_INPUTS)
	$(INIT_BUILD_COMMAND)
```

If you add a source root to your init project, add it to `INIT_INPUTS` too, or
that root becomes invisible to the dependency graph.

### nvmed and lpcid

`nvmed` is the userspace NVMe block service from the `nvmed-rs` checkout: a
`no_std` Rust program whose own `makefile` runs Cargo for the
`x86_64-unknown-none` target and produces `build/nvmed.bin`. `lpcid` remains a C
program. Both are out-of-tree user programs, embedded as `/bin/nvmed` and
`/bin/lpcid`. Both are located by the `NVMED_*` and `LPCID_*` keys described
above, and both are rebuilt by a recursive `make`:

```make
NVMED_BIN := $(NVMED_DIR)/build/$(NVMED_BIN_NAME).bin
NVMED_INPUTS := $(shell find $(NVMED_DIR)/src $(NVMED_DIR)/include -type f 2>/dev/null) \
                $(NVMED_DIR)/makefile $(NVMED_DIR)/linker.ld \
                $(wildcard $(NVMED_DIR)/Cargo.toml $(NVMED_DIR)/Cargo.lock)

$(NVMED_BIN): $(NVMED_INPUTS)
	@$(MAKE) -C $(NVMED_DIR)

$(NVMED_BLOB_O): $(NVMED_BLOB_S) $(NVMED_BIN)
	$(AS) $(ASFLAGS) -DNVMED_BIN=\"$(abspath $(NVMED_BIN))\" $< -o $@
```

`lpcid` is identical with `LPCID` substituted. The `-D` is load-bearing in both.
An `incbin` of a relative path is resolved by `nasm` relative to the `.s` file and
carries no timestamp, so the blob object stays valid no matter what the binary on
disk contains - which is how an edited `nvmed` kept booting as its previous build.
Passing the absolute path through a define ties the object to the binary, and the
binary to its sources.

Watch the variable names when copying one block to the other. A `LPCID_INPUTS:`
line that mistakenly assigns `NVMED_INPUTS` compiles fine and leaves `lpcid` with
no prerequisites at all, so it is rebuilt only when the file is missing and
`lpcid.bin` edits silently boot the previous build.

`SHELL_BIN` has no prerequisites at all, so it is rebuilt only when it is
missing. That is fine for the out-of-tree shell, which has no source vendored
here and is rebuilt by hand; it is not fine for an in-tree program. When you add
a user program to this tree, give its binary explicit prerequisites (see
`USER_HELLO_BIN` below) rather than relying on `make` noticing the change.

## Targets

```sh
make          # build/os.img
make run      # boot it in QEMU
make debug    # boot paused, GDB stub on :1234
make clean    # remove build artifacts and preserve the disk images
```

`make` rebuilds the init binary if any of its sources changed, then rebuilds any
kernel object whose blob changed, then assembles the image. Outputs:

| file | what |
|------|------|
| `build/boot.bin` | MBR |
| `build/stage_1.bin` | 4 sectors |
| `build/stage_2.bin` | 32 sectors |
| `build/kernel.elf` | kernel with symbols, for GDB |
| `build/kernel.bin` | flat kernel image |
| `build/os.img` | the whole disk image |

Image assembly and padding:

```make
cat $(BOOT) $(STAGE1) $(STAGE2) $(KERNEL) > $@
truncate -s $$(( ( $(KERNEL_LBA) + $(KERNEL_SECTORS) ) * 512 )) $@
```

## Emulated hardware

`run` and `debug` default to four virtual CPUs and a writable NVMe namespace
backed by the 64 MiB FAT32 image at `build/fat32.img`. The build uses
`dosfstools` to create the default image when it is missing. `make fat-image`
creates that image without starting QEMU.

Set `QEMU_CPUS` to choose a CPU count. Set `QEMU_NVME_IMAGE` to attach a
different image, such as `make run QEMU_NVME_IMAGE=build/nvme.img` to use the
original test disk. If an alternate image path is missing, the build creates
a blank 64 MiB raw image there.

```make
FAT_IMAGE ?= $(BUILD)/fat32.img
QEMU_NVME_IMAGE ?= $(FAT_IMAGE)
QEMU_CPUS ?= 4
QEMU_DEVICES := \
	-drive if=none,id=nvme0,file=$(QEMU_NVME_IMAGE),format=raw \
	-device nvme,drive=nvme0,serial=lumen0
```

`if=none` attaches the image exclusively to the NVMe controller. Writes to the
namespace persist in the image across runs.

`serial` is required. QEMU 11 refuses to start an `nvme` device without one:

```
qemu-system-x86_64: -device nvme,drive=nvme0: serial property not set
```

Do not drop it when overriding `QEMU_DEVICES` on the command line.

## Toolchains

User programs in this tree are C.

**C** uses `USER_CFLAGS`:

```
-ffreestanding -m64 -mno-red-zone -fno-stack-protector -fno-pie -fno-pic
-ffunction-sections -fdata-sections -Wall -Wextra -Iinclude
```

**`-mno-red-zone` is mandatory.** `switch_context` pushes the `iretq` frame below
the saved user RSP, so a red zone would be clobbered across a context switch.

`-C no-redzone=yes` is the Rust equivalent of the same requirement.

## Flat binaries

User programs are linked for a fixed base, converted with `objcopy -O binary`,
embedded via `incbin`, and published into ramfs at boot.

### Offset 0 must be the entry point

Lumen loads a user image at its link base and enters at *that same address*, so
the flat binary has to begin with `_start`. This only holds if the custom linker
script is actually applied with `-T`. With `ld`'s default script the sections get
page-aligned (`_start` lands at `0x40185b`), GNU note sections are emitted first,
and offset 0 becomes note bytes instead of code. `/DISCARD/ { *(.note*) }` does
not help there - it is never consulted if the script itself is not used.

The failure is easy to miss because the note decodes as harmless-looking
instructions before it dies:

```
00400000: 04 00   add al, 0x0            <- runs fine
00400002: 00 00   add byte [rax], al     <- RAX = 0, writes to linear 0, faults
```

which shows up as a `#GP` two bytes into what looks like a prologue:

```
RIP=0000000000400002  RA=0000000000000000
```

Quick check after any linker change:

```sh
xxd -l 16 build/user/prog.bin
# C _start:    55 48 89 e5 48 83 ec ..   (push rbp; mov rbp,rsp; sub ...)
# Rust _start: 50 e8 ..                  (push rax; call ...)
# note header: 04 00 00 00 14 00 00 00 03 00 00 00 ...   <- wrong
```

### Linker scripts

`src/user/ld/` holds one script, for the `0x500000` exec slot:

| script | base | used by |
|--------|------|---------|
| `prog.ld` | `0x500000` | `exec` targets, including `hello` |

It puts `*(.text._start)` first and discards notes, so as long as it is passed
with `-T` the entry point lands at offset 0.

The init and shell projects each carry their own linker script for `0x400000`
(`init.ld` and `shell.ld` respectively). Those are outside this tree, so if you
are changing how a user program is linked, change the script next to that
project, not here.

## Writing your own user program (can change in next update)

1. Start from `src/user/hello/hello.c`. Freestanding C
   only: `syslib.h` (`sys_*`, `ustr*`, `uitoa`), entry `void _start(void)`,
   finish with `sys_exit(code)`. No libc. `.bss` starts zero, so don't rely on
   nonzero initialisers.
2. Add `USER_<NAME>_*` rules to the `makefile` (copy the `USER_HELLO_*` block),
   linking with `src/user/ld/prog.ld` **via `-T`**. Add
   `src/user/blobs/<name>_blob.s` (copy `hello_blob.s`, point `incbin` at your
   `.bin`). Give the `.bin` a rule listing its sources - an empty rule like
   `$(USER_<NAME>_BIN):` rebuilds only when the file is missing, so source edits
   silently boot the previous binary. Also add the object as a prerequisite of
   its blob:

   ```make
   $(BUILD)/user/blobs/<name>_blob.o: $(USER_<NAME>_BIN)
   ```
3. Publish it in `kload_user_bins()` (`src/kernel/kernel.c`) with `vfs_create` +
   `vfs_write`, e.g. as `/bin/<name>`.
4. `make && make run`, then `run <name>` in the shell.
5. Keep the flat binary under 64 KiB (`PROG_MAX`).

## See also

[Boot chain](boot.md), [userspace](userspace.md), [debugging](debugging.md).

## Rust in the kernel

Install Rust 1.85 or later with the `x86_64-unknown-none` target
(`rustup target add x86_64-unknown-none`). The normal `make` build invokes Cargo
in `src/kernel/rust` (with its target directory under `build/kernel/cargo`) and
copies the resulting staticlib to `build/kernel/liblumen_rust.a` before linking
it into `kernel.elf`. The crate uses `core`, static addressing and aborting
panics. The target uses a soft-float ABI and the build disables the red zone.
A Rust panic reports a fatal error through the C console and halts the CPU.

`cpuinfo.rs` uses the `no_std` `raw-cpuid` crate to collect CPUID data at boot:
vendor, brand string, family/model/stepping, APIC IDs and APIC features, plus
physical-core and logical-processor counts with the leaf that supplied them.
CPUID only describes the boot processor's package; system-wide core enumeration
requires ACPI/MP table parsing, which the kernel does not do yet.

Use safe Rust interfaces from kernel code. For example, `BootConsole` takes
borrowed strings and implements `core::fmt::Write` for allocation-free
formatting. Its unsafe constructor requires exclusive console access. The
C boot call meets that contract before interrupts and scheduling start.
Keep the handle within that boot phase.

- Add C imports to `src/kernel/rust/bindings.rs` and mirror the C signatures
  there. For C macros or inline functions, export a small C bridge function.
  Put raw pointer, lifetime and execution-context checks at the boundary and
  expose safe functions only when their invariants can be enforced.
- Export Rust functions with `extern "C"` and `#[unsafe(no_mangle)]` and add
  matching declarations to `include/lumen/kernel/rust.h`. Use fixed-width
  integers, pointers, and `#[repr(C)]` structs where needed. Document any
  caller obligations in both languages.

During boot, `Lumen Rust kernel ready.` verifies the C entry into Rust and
the Rust call back into the C console.
