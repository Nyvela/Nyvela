# Build system

## Requirements

* `gcc` with x86-64 support and freestanding flags
* `nasm`
* `ld`, `objcopy`
* `cargo` + `rustc` with the `x86_64-unknown-none` target (for the Rust shell)
* `make`
* `qemu-system-x86_64`

Tested on Linux with GCC 13+ and QEMU 8+.

## nyvela.conf

Nyvela is kernel-only. There is no userspace init program in the tree, but a
kernel needs something to boot into in order to be exercised, so `make` reads
`nyvela.conf` from the repo root and stops immediately if it is missing or
incomplete. Example config:

```make
INIT_DIR = ../Nyvd
INIT_BIN_NAME = nyvd
INIT_BUILD_COMMAND = USER_CFLAGS="-I$(CURDIR)/include" make -C $(INIT_DIR)
```

| key | meaning |
|-----|---------|
| `INIT_DIR` | directory of the init project - must exist, `make` errors out otherwise |
| `INIT_BIN_NAME` | base name of its flat binary; `make` expects `$(INIT_DIR)/build/$(INIT_BIN_NAME).bin` |
| `INIT_BUILD_COMMAND` | command that rebuilds it; runs before every kernel object so blob edits are picked up |

The resulting binary is embedded with `incbin` (`src/user/blobs/init_blob.s`)
and published as `/bin/init` at boot. Nothing from that project is expected to
end up in this repository; it only has to produce a working flat binary.

`INIT_BUILD_COMMAND` must forward `USER_CFLAGS`, if the init project
includes Nyvela headers (`<nyvela/user/syslib.h>`) and only learns the include
path from this invocation. Building it by hand without that flag fails with
`fatal error: nyvela/user/syslib.h: No such file or directory`.

## Targets

```sh
make          # build/os.img
make run      # boot it in QEMU
make debug    # boot paused, GDB stub on :1234
make clean    # rm -rf build/
```

`make` runs `INIT_BUILD_COMMAND` first, then rebuilds any kernel object whose
blob changed. Outputs:

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

Both C and Rust user programs are supported.

**C** uses `USER_CFLAGS`:

```
-ffreestanding -m64 -mno-red-zone -fno-stack-protector -fno-pie -fno-pic
-ffunction-sections -fdata-sections -Wall -Wextra -Iinclude
```

**`-mno-red-zone` is mandatory.** `switch_context` pushes the `iretq` frame below
the saved user RSP, so a red zone would be clobbered across a context switch.

**Rust** builds `no_std` for `x86_64-unknown-none` with:

```
-C link-arg=-T<ld script> -C link-arg=-nostdlib -C link-arg=-static
-C link-arg=-no-pie -C link-arg=--gc-sections -C no-redzone=yes
```

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

`src/user/ld/` has one per load address:

| script | base | used by |
|--------|------|---------|
| `init.ld` | `0x400000` | the init program |
| `shell.ld` | `0x400000` | the Rust shell |
| `prog.ld` | `0x500000` | `exec` targets |

Each puts `*(.text._start)` first and discards notes, so as long as it is passed
with `-T` the entry point lands at offset 0.

## Writing your own user program (can change in next update)

1. Start from `src/user/hello/hello.c`. Freestanding C
   only: `syslib.h` (`sys_*`, `ustr*`, `uitoa`), entry `void _start(void)`,
   finish with `sys_exit(code)`. No libc. `.bss` starts zero, so don't rely on
   nonzero initialisers.
2. Add `USER_<NAME>_*` rules to the `makefile` (copy the `USER_HELLO_*` block),
   linking with `src/user/ld/prog.ld` **via `-T`**. Add
   `src/user/blobs/<name>_blob.s` (copy `hello_blob.s`, point `incbin` at your
   `.bin`).
3. Publish it in `kload_user_bins()` (`src/kernel/kernel.c`) with `vfs_create` +
   `vfs_write`, e.g. as `/bin/<name>`.
4. `make && make run`, then `run <name>` in the shell.
5. Keep the flat binary under 64 KiB (`PROG_MAX`).

## See also

[Boot chain](boot.md), [userspace](userspace.md), [debugging](debugging.md).
