# Userspace

## Threads and address spaces

The scheduling unit is a `thread_t` (`src/thread/thread.c`), identified by a
`tid`. The kernel starts a ring0 idle thread and ring3 init. Legacy services
use the native address-space and thread syscalls. ELF programs get a process
record with a private address space and descriptor table.

A thread carries its address space directly - `thread_t.context->cr3` is the
root it runs under, so two threads in the same `cr3` share memory and two
threads in different `cr3`s do not. Legacy address spaces come from
`kvmm_create_user_pml4()`. ELF address spaces start from the kernel's
supervisor mappings and add private user pages.

Legacy user pages are mapped with flags `0x07` (present, writable, user), and
`kvmmap_at` propagates `U/S` up the path tables.

The ring0 idle thread waits for interrupts and polls the network stack.

## Legacy memory layout

Constants live in `include/lumen/mm/vmm.h` and are mirrored in `syslib.h`:

| symbol | address | what |
|--------|---------|------|
| `USER_CODE_VIRT` | `0x400000` | init / shell image |
| `USER_HEAP_BASE` | `0x410000` | first page handed out by `alloc_page` |
| `USER_STACK_TOP` | `0x450000` | init/shell stack top |
| `USER_STACK_PAGE` | `0x44F000` | init/shell stack page |
| `PROG_BASE` | `0x500000` | `exec` target slot (`PROG_PAGES` = 16) |
| `PROG_STACK_TOP` | `0x600000` | program stack top |
| `PROG_STACK_PAGE` | `0x5FF000` | highest program stack page, with seven additional pages below it for an exec child |
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

The init program (the external project configured in `lumen.conf`, see
[build system](build.md)) then does the same job for `/bin/sh`:

1. `alloc_page` per page of the shell image.
2. Read the image in with `fs_read` in 2 KiB chunks.
3. `as_create` a new address space, `as_map` the stack page and each code page.
4. `create_thread` at `USER_CODE_VIRT` with `rsp = USER_STACK_PAGE + 4096`.

`exec` (`SYS_EXEC`) runs foreground programs from RAMFS. Flat programs use
the `PROG_BASE` slot. ELF programs use their segment addresses and receive
arguments from the shell. For either format the caller waits for the child,
then reaps it and receives its exit code.

## Shell

Ring3, a `no_std` Rust program built from its own project (`SHELL_DIR` in
`lumen.conf`, see [build system](build.md)), prompt `Lumen > `, with line
editing (echo + backspace).

| command | effect |
|---------|--------|
| `help` | list commands |
| `ls [path]` | list a directory (default `/`, relative paths work) |
| `cat <file>` | print a file |
| `echo <...>` | print arguments |
| `run <name> [args...]` | run `/bin/<name>` in the foreground, pass quoted ELF arguments and print `exit: <code>` |
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

* Lumsh runs one foreground program at a time and has no pipes or job control.
* The parent reaps foreground exec children. Legacy services manage other
  thread lifetimes through their native API.
* See [Static ELF applications](ports.md) for the ELF ABI and supported
  toolchains.

## See also

[syscalls](syscall.md), [build system](build.md), [debugging](debugging.md).
