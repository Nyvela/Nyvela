# Userspace

## Threads and address spaces

There is no process layer. The scheduling unit is a `thread_t`
(`src/thread/thread.c`), identified by a `tid`. The kernel spawns exactly two of
them - a ring0 idle thread and the init thread - then hands control to ring3 and
never constructs another. From that point on **the init program is responsible
for creating everything else**, via syscalls.

A thread carries its address space directly - `thread_t.context->cr3` is the
root it runs under, so two threads in the same `cr3` share memory and two
threads in different `cr3`s do not. Fresh ring3 address spaces come from
`kvmm_create_user_pml4()`, which is what `as_create` (`30`) hands out and what
`exec` (`20`) builds per child.

User pages are mapped with flags `0x07` (present, writable, user), and
`kvmmap_at` propagates `U/S` up the path tables.

The ring0 idle thread just runs `hlt` in a loop.

## Memory layout

Constants live in `include/nyvela/mm/vmm.h` and are mirrored in `syslib.h`:

| symbol | address | what |
|--------|---------|------|
| `USER_CODE_VIRT` | `0x400000` | init / shell image |
| `USER_HEAP_BASE` | `0x410000` | first page handed out by `alloc_page` |
| `USER_STACK_TOP` | `0x450000` | init/shell stack top |
| `USER_STACK_PAGE` | `0x44F000` | init/shell stack page |
| `PROG_BASE` | `0x500000` | `exec` target slot (`PROG_PAGES` = 16) |
| `PROG_STACK_TOP` | `0x600000` | program stack top |
| `PROG_STACK_PAGE` | `0x5FF000` | program stack page |
| `USER_AREA_BASE` / `USER_AREA_END` | `0x400000` / `0x700000` | syscall pointer validity window |

## Bootstrap

The kernel does the first steps itself in `kuserspace_init()`:

1. Spawn the ring0 idle thread.
2. Spawn the init thread with entry `USER_CODE_VIRT`.
3. Allocate and map pages for its image, then load `/bin/init` into them with
   `exec_load`.
4. Map its stack, the `exec` program slot, and the program stack.
5. `switch_context` into ring3 with `cs=0x1B`, `ss=0x23`, `rsp=USER_STACK_TOP`.

It never comes back; from that point on everything is userspace.

The init program (the external project configured in `nyvela.conf`, see
[build system](build.md)) then does the same job for `/bin/sh`:

1. `alloc_page` per page of the shell image.
2. Read the image in with `fs_read` in 2 KiB chunks.
3. `as_create` a new address space, `as_map` the stack page and each code page.
4. `create_thread` at `USER_CODE_VIRT` with `rsp = USER_STACK_PAGE + 4096`.

`exec` (`SYS_EXEC`) does the same thing for foreground programs: it spawns a
child thread in a fresh address space, re-maps the `PROG_BASE` slot onto fresh
pages, reads the file in, runs it ring3, blocks on `sti/hlt` until the child
dies, reaps it, and returns its exit code.

## Shell

Ring3, a `no_std` Rust program built from its own project (`SHELL_DIR` in
`nyvela.conf`, see [build system](build.md)), prompt `Nyvela > `, with line
editing (echo + backspace).

| command | effect |
|---------|--------|
| `help` | list commands |
| `ls [path]` | list a directory (default `/`, relative paths work) |
| `cat <file>` | print a file |
| `echo <...>` | print arguments |
| `run <name>` | run `/bin/<name>` in the foreground, print `exit: <code>` |
| `clear` | clear the screen |
| `exit` | exit the shell |

Paths resolve against `/` with trailing slashes stripped. Errors print readably
(`run test` -> `run: /bin/test: no such file or directory`).

## IPC

Queued messages only. `SYS_IPC_SEND` (`5`) takes a target `tid` and copies the
bytes into that thread's per-thread queue; `SYS_IPC_POLL` (`6`) pops one and
copies it back to a user buffer, returning the byte count (`0` on failure).
Queues grow on demand (`ipc_grow_queue`) and are freed with their thread. There
is no blocking receive - `poll` returns immediately with `0` when the queue is
empty.

## Known limitations

* The exec slot is single-use: one foreground program at a time, no arguments,
  no pipes, no background jobs.
* Nothing reaps a thread that exits outside `exec`; `DEAD` threads linger and
  the scheduler skips them.
* User pointers are range-checked but not ownership-checked.
* No ELF loader - flat binaries only, entry at a fixed base.

## See also

[syscalls](syscall.md), [build system](build.md), [debugging](debugging.md).
