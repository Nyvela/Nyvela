# Userspace

## Processes and address spaces (to be removed)

`spawn_process()` (`src/process/process.c`) allocates a `process_t`, gives it a
PID, and creates a **fresh user PML4** with `kvmm_create_user_pml4()`. Every
process therefore has its own address space; user pages are mapped with flags
`0x07` (present, writable, user), and `kvmmap_at` propagates `U/S` up the path
tables.

Each process owns one `thread_t` today. The ring0 idle process just runs `hlt`
in a loop.

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

1. Spawn the ring0 idle process.
2. Spawn the init process with entry `USER_CODE_VIRT`.
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
child process, re-maps the `PROG_BASE` slot onto fresh pages, reads the file in,
runs it ring3, blocks on `sti/hlt` until the child dies, reaps it, and returns
its exit code.

## Shell

Ring3, written in Rust (`src/user/shell/`),
prompt `Nyvela > `, with line editing (echo + backspace).

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

Scaffold only. `SYS_IPC` (`5`) takes a target PID and copies bytes into
`USER_AREA_BASE + 0x1F0` in every matching process. There is no receive side,
no queueing, and the scratch address sits inside the target's first code page,
so it clobbers the target's image. Treat it as a placeholder for the interface,
not a working feature.

## Known limitations

* The exec slot is single-use: one foreground program at a time, no arguments,
  no pipes, no background jobs.
* No process reaping for threads that exit outside `exec`; `DEAD` threads linger
  and the scheduler skips them.
* User pointers are range-checked but not ownership-checked.
* No ELF loader - flat binaries only, entry at a fixed base.

## See also

[syscalls](syscall.md), [build system](build.md), [debugging](debugging.md).
