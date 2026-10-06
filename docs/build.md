# Build system

## Requirements

* `gcc` with x86-64 support and freestanding flags
* `nasm`
* `ld`, `objcopy`
* `make`
* `qemu-system-x86_64`

Tested on Linux with GCC 13+ and QEMU 8+.

## nyvela.conf

Nyvela is kernel-only. Neither the init program nor the shell is in this tree,
but the kernel needs something to boot into in order to be exercised, so `make`
reads `nyvela.conf` from the repo root and stops immediately if it is missing or
incomplete. Six keys, in two groups:

```make
INIT_DIR = ../Nyvd
INIT_BIN_NAME = nyvd
INIT_BUILD_COMMAND = USER_CFLAGS="-I$(CURDIR)/include" make -C $(INIT_DIR)

SHELL_DIR = ../nyvsh
SHELL_BIN_NAME = shell
SHELL_BUILD_COMMAND = make -C $(SHELL_DIR)
```

| key | meaning |
|-----|---------|
| `INIT_DIR` | directory of the init project - must exist, `make` errors out otherwise |
| `INIT_BIN_NAME` | base name of its flat binary; `make` expects `$(INIT_DIR)/build/$(INIT_BIN_NAME).bin` |
| `INIT_BUILD_COMMAND` | command that rebuilds it; runs before every kernel object so blob edits are picked up |
| `SHELL_DIR` | directory of the shell project |
| `SHELL_BIN_NAME` | base name of its flat binary; `make` expects `$(SHELL_DIR)/build/$(SHELL_BIN_NAME).bin` |
| `SHELL_BUILD_COMMAND` | command that rebuilds it |

Both binaries are embedded with `incbin` (`src/user/blobs/init_blob.s` and
`shell_blob.s`, via `-DINIT_BIN=` and `-DSHELL_BIN=`) and published as `/bin/init`
and `/bin/sh` at boot. Nothing from either project is expected to end up in this
repository; each only has to produce a working flat binary.

`INIT_BUILD_COMMAND` must forward `USER_CFLAGS`, if the init project includes
Nyvela headers (`<nyvela/user/syslib.h>`) and only learns the include path from
this invocation. Building it by hand without that flag fails with
`fatal error: nyvela/user/syslib.h: No such file or directory`.

### How staleness is tracked

The init binary is a prerequisite of every kernel object, but on its own that is
not enough: `make` only reruns `$(INIT_BUILD_COMMAND)` when a listed prerequisite
is newer than the target. Depending on `nyvela.conf` alone means editing init
sources does nothing, `nyvd.bin` keeps its old timestamp, and `incbin` embeds
the previous build. The symptom is a kernel that boots a feature you already
wrote - a new spawn target, a new `/bin` entry - and silently keeps the old
behaviour, with no error anywhere.

So the rule also depends on the init project's own sources:

```make
INIT_INPUTS := $(shell find $(INIT_DIR)/src $(INIT_DIR)/include -type f 2>/dev/null) \
               $(INIT_DIR)/makefile $(INIT_DIR)/linker.ld

$(INIT_BIN): nyvela.conf $(INIT_INPUTS)
	$(INIT_BUILD_COMMAND)
```

If you add a source root to your init project, add it to `INIT_INPUTS` too, or
that root becomes invisible to the dependency graph.

### nvmed

`nvmed` is a second out-of-tree user program, embedded as `/bin/nvmed`. It has no
`nyvela.conf` keys; the makefile defaults locate it and can be overridden:

```make
NVMED_DIR ?= ../nvmed
NVMED_BIN_NAME ?= nvmed
NVMED_INPUTS := $(shell find $(NVMED_DIR)/src $(NVMED_DIR)/include -type f 2>/dev/null) \
                $(NVMED_DIR)/makefile $(NVMED_DIR)/linker.ld

$(NVMED_BIN): $(NVMED_INPUTS)
	@$(MAKE) -C $(NVMED_DIR)

$(NVMED_BLOB_O): $(NVMED_BLOB_S) $(NVMED_BIN)
	$(AS) $(ASFLAGS) -DNVMED_BIN=\"$(abspath $(NVMED_BIN))\" $< -o $@
```

The `-D` is load-bearing. An `incbin` of a relative path is resolved by `nasm`
relative to the `.s` file and carries no timestamp, so the blob object stays
valid no matter what the binary on disk contains - which is how an edited nvmed
kept booting as its previous build. Passing the absolute path through a define
ties the object to the binary, and the binary to its sources.

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
make clean    # rm -rf build/
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

Nyvela loads a user image at its link base and enters at *that same address*, so
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
