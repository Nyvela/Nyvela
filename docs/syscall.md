# Syscalls

Everything user programs can reach goes through `int $0x80`.

## ABI

| register | role |
|----------|------|
| `rax` | syscall number on entry, return value on exit |
| `rdi`, `rsi`, `rdx`, `r10` | arguments 1..4 |
| `rcx`, `r11` | clobbered |

Returns are byte counts on success, or a negative `VFS_ERR_*` cast to `u64` on
failure.

| error | meaning |
|-------|---------|
| `VFS_ERR_INVAL` | bad argument |
| `VFS_ERR_NOTFOUND` | no such file |
| `VFS_ERR_EXISTS` | already exists |
| `VFS_ERR_NOTDIR` / `VFS_ERR_ISDIR` | wrong node type |
| `VFS_ERR_NOSPACE` | too large / out of room |
| `VFS_ERR_BADPATH` | malformed path |

## Table

| nr | name | args | effect |
|----|------|------|--------|
| 0 | `exit` | `code=rdi` | records exit code, marks the thread `DEAD`, switches away (never returns) |
| 1 | `write` | `fd`, `buf`, `len` | `fd` 1/2 to the VGA console; handles `\n` and `\b`; `len <= 2048` |
| 2 | `yield` | - | no-op, returns 0 (preemption is timer-driven) |
| 3 | `read` | `fd` (0), `buf`, `len` | blocking keyboard read; waits with `sti/hlt` until at least one byte |
| 4 | `clear` | - | clears the console |
| 5 | `ipc_send` | `target_tid`, `buf`, `size` | queues an IPC message on the target thread's queue |
| 6 | `ipc_poll` | `buf`, `size` | pops one IPC message from the caller's queue and copies `size` bytes to `buf`, returns the amount of bytes written. 0 indicates failure or an empty queue |
| 10 | `fs_create` | `path`, `is_dir=rsi` | `vfs_create` |
| 11 | `fs_write` | `path`, `buf`, `len`, `offset=r10` | `vfs_write` |
| 12 | `fs_read` | `path`, `buf`, `len`, `offset=r10` | `vfs_read` |
| 13 | `fs_list` | `path`, `buf`, `len` | newline-separated names plus NUL, returns bytes excluding NUL |
| 14 | `fs_size` | `path` | size in bytes |
| 20 | `exec` | `path` | runs a ramfs file in the `0x500000` slot in a child thread with a fresh address space, blocks until exit, returns its exit code; file must be `<= PROG_MAX` |
| 21 | `create_thread` | `entry`, `as_id`, `rsp` | spawns a ring3 thread in the given address space, returns `tid` |
| 22 | `free_thread` | `tid` | frees a thread handle |
| 30 | `as_create` | - | creates an address space, returns its id |
| 31 | `as_map` | `as_id`, `virt`, `phys_virt`, `flags` | maps the caller's page at `phys_virt` into `virt` of another address space |
| 32 | `as_free` | `as_id` | frees a user PML4 |
| 40 | `alloc_page` | - | allocates and maps a free user page from `USER_HEAP_BASE`, returns the virtual address |
| 41 | `free_page` | `virt` | unmaps and frees a user page |
| 42 | `mmap` | `virt`, `phys`, `flags` | maps a page in the caller's address space |
| 43 | `unmmap` | `virt` | unmaps a page in the caller's address space |
| 44 | `unmmap_and_free` | `virt` | unmaps and frees a page in the caller's address space |

## Validation

User pointers must lie within `USER_AREA_BASE`..`USER_AREA_END`
(`0x400000`..`0x700000`) and stay inside the length bound. Paths must be
NUL-terminated within `SYSCALL_MAX_PATH` (256) bytes, buffers are capped at
`SYSCALL_MAX_BUF` (2048). Each syscall copies through a `kmalloc` bounce buffer
rather than touching user memory directly.

The range check exists because user threads currently share one flat layout, so a
bad pointer is only caught by range, not by ownership. Once address spaces are
properly isolated this should be tightened to per-space validation.

## Interrupts

`0x80` is an interrupt gate, so it runs with `IF=0`, which makes fast syscalls
atomic against the timer. The blocking ones (`read`, `exec`) re-enable
interrupts and `hlt` so preemption keeps working - safe because every thread has
its own kernel stack and `TSS.rsp0` is repointed at whichever thread is
scheduled.

## User SDK

`include/nyvela/user/syslib.h` has the syscall numbers, inline wrappers
(`sys_exit`, `sys_write`, `sys_read`, `sys_yield`, `sys_fs_*`, `sys_exec`), a
raw `sys_call(nr, a1..a4)`, and string helpers (`ustrlen`, `ustrcmp`,
`ustrncmp`, `umemcpy`, `umemset`, `uitoa`).

## See also

[userspace](userspace.md), [build system](build.md).
