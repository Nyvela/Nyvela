# Lumen native application calls

ELF processes use the syscall numbers and types in `sdk/abi.rs`.
`sdk/sys.rs` provides a Rust trap wrapper for x86-64. The kernel checks user
mappings before reading or writing application buffers. Calls return a
signed 64-bit result. Negative results use errno values.

## Files and memory

| Operation | Arguments and result |
|-----------|----------------------|
| `OPEN` | NUL-terminated path, `OPEN_*` flag mask, `CURRENT_DIRECTORY`. Returns a file handle. |
| `CLOSE` | Handle. |
| `READ`, `WRITE` | Handle, buffer pointer, byte length. Return bytes transferred. |
| `READ_AT`, `WRITE_AT` | Handle, buffer pointer, byte length, byte offset. |
| `INFO_PATH`, `INFO_HANDLE` | Path or handle, writable `FileInfo` pointer, directory selector for a path. |
| `READ_DIRECTORY` | Handle and writable `DirectoryEntry` pointer. Returns one entry or zero at end. |
| `MAP_MEMORY` | Optional address hint, byte length, protection bits and placement flags. Returns an anonymous mapping. Protection bits 1, 2 and 4 mean read, write and execute. |
| `UNMAP_MEMORY`, `PROTECT_MEMORY` | Mapping address and byte length, followed by protection bits for `PROTECT_MEMORY`. |
| `SET_TLS`, `GET_TLS` | FS base value or writable pointer to the base. |
| `UPTIME`, `SLEEP` | Uptime returns milliseconds since boot. Sleep takes a millisecond duration. |

## Processes and sessions

| Operation | Arguments and result |
|-----------|----------------------|
| `SPAWN` | ELF path, packed arguments, packed byte length, requested UID, creation flags. Returns a child ID. |
| `WAIT` | Child ID and nonblocking flag. Returns the exit status. A pending nonblocking wait returns `-EAGAIN`. |
| `EXIT` | Exit status. Descendants are destroyed when the process exits. |
| `SELF` | Returns the caller’s process ID. |
| `IDENTITY` | Selector 0 returns UID, 1 returns the administrator attribute and 2 returns parent ID. |
| `ACCOUNT_REGISTER` | Pointer to an `Account` record. Available to the system account. |
| `ACCOUNT_DESCRIBE` | UID and writable pointer to an `Account` record. |

`SPAWN` accepts up to 512 bytes of NUL-terminated arguments. The kernel
creates `argv[0]` from the path. A requested UID of `u64::MAX` inherits the
caller’s identity. The system account can select a registered UID.

Creation flag 1 delegates the display seat when the caller has that seat.
Flag 2 forwards stdout and stderr as IPC messages to the parent and supplies
EOF on stdin. The sender ID identifies the child. The graphical terminal
uses this mode for batch applications.

## IPC and display

`SEND` takes a target process ID, byte pointer and length. `RECEIVE` takes
an output buffer, its capacity and an optional pointer for the sender ID.
Messages are limited to 4,112 bytes. A receive returns the byte count or
zero when the queue is empty. Regular users send to processes with their
own UID.

`DISPLAY_ACQUIRE` reserves the display for the caller. The owner can
submit a full 640×400 packed XRGB frame through `DISPLAY_PRESENT`. The
owner also reads PS/2 events and releases the display when done.
`DISPLAY_KEY` returns a set-1 scancode with bit 8 set for key-down events
and bit 9 for extended events. `DISPLAY_POINTER` writes a `Pointer`
record to the specified buffer and returns 1. Both return zero when no
event is waiting. The kernel releases the seat on process exit.
