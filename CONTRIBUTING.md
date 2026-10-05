# Contributing

Thanks for poking at it. This is a small OS with a lot of sharp edges, so the
notes below are mostly about not wasting an afternoon to a bootloader
misunderstanding.

## Getting set up

You need `gcc`, `nasm`, `ld`, `objcopy`, `make`, and `qemu-system-x86_64`.

`make` will not run without a `nyvela.conf`. Nyvela is kernel-only, so the
config points at the two userspace programs it boots into, both outside the
tree:

Example config:

```make
INIT_DIR = ../Nyvd
INIT_BIN_NAME = nyvd
INIT_BUILD_COMMAND = USER_CFLAGS="-I$(CURDIR)/include" make -C $(INIT_DIR)

SHELL_DIR = ../nyvsh
SHELL_BIN_NAME = shell
SHELL_BUILD_COMMAND = make -C $(SHELL_DIR)
```

Then `make && make run`. You should reach a `Nyvela >` prompt.

## Ground rules

- **Check it boots.** A change is not done until `make run` still reaches the
  shell prompt. `make debug` gives you GDB on `:1234` if it does not.
- **Keep commits focused.** One logical change per commit, and keep unrelated
  reformatting out of feature commits - it makes the diff reviewable.
- **Match the surrounding style.** Indentation, naming, comment density. No
  reformatting unrelated lines in a patch.
- **Don't add dependencies.** The kernel is freestanding C and NASM. User
  programs are freestanding C too, or a `no_std` Rust crate in their own
  project. No libc, no host headers, no runtime.

## Commit messages

Conventional-commit prefixes, lowercase, short imperative subject:

```
feat: spawn_process in userspace
fix: context switch stack corruption
chore: minor clean up
```

`feat`, `fix`, `chore`, and `docs` are all in use. The subject line should say
what changed, not what you did.

## Tests

There is no host-side test runner. The self-tests live in `src/kernel/ktests.c`
and run at boot, printing `[ SUCC ]` or `[ FERR ]` per test:

| test | covers |
|------|--------|
| `ktest_palloc` | PMM page allocation |
| `ktest_vmmap` | 4 KiB map/unmap |
| `ktest_kmalloc` | heap allocation |
| `ktest_krealloc` | heap resize |
| `ktest_vfs` | ramfs create/write/read/list |
| `ktest_syscall` | the `int $0x80` path from ring0 |

If your change touches one of those subsystems, add or extend a test there and
make sure the whole set still passes. The boot screen is the report, there is
nothing to assert on.

## Things that will bite you

**A user flat binary must start with `_start` at offset 0.** Nyvela loads an
image at its link base and enters at that same address. That only holds if the
custom linker script is actually applied with `-T`. Without it, `ld` page-aligns
the sections and emits GNU notes first, so offset 0 is a note and the program
faults two bytes into what looks like a prologue. If you touch a linker script
or a user program's build flags, `xxd -l 16` the `.bin` and check.

**No red zone, anywhere.** `switch_context` pushes the `iretq` frame below the
saved user RSP, so red-zone locals are clobbered across a context switch. C
needs `-mno-red-zone` (it is in `USER_CFLAGS`), Rust needs `-C no-redzone=yes`
(it is in the shell project's cargo `RUSTFLAGS`). Please do not remove either.

**Faults are log-and-halt.** There is no recovery and no IST yet, so a ring3
fault stops the machine. That is expected behaviour right now, not a new bug -
but it also means you cannot test recovery paths that do not exist.

## Pull requests

- Open an issue first for anything larger than a fix, so the approach can be
  agreed before you write it.
- PRs get merged, not squashed away silently - rebase on `main` if `main` has
  moved.
- Say what you tested and what you did not. "Boots to the shell, did not test
  SMP" is genuinely useful.

## Reporting bugs

The boot console and QEMU together carry most of the diagnosis:

- The exact console output, including the `[ FERR ]` line if there is one.
- `qemu-system-x86_64 -d int,cpu_reset,guest_errors -D qemu.log` output if it
  resets or faults.
- The register dump from the fault handler (vector, error code, RIP, and the
  bytes at RIP) if one fired.

[docs/debugging.md](docs/debugging.md) has recipes for all three, including a
table mapping common symptoms to causes.

## License

Contributions are accepted under GPL-3.0, the same as the project. See
[LICENSE](LICENSE).
