# Static ELF applications

Lumen ELF programs enter the kernel through the native `int 0x80` interface.
The Lumen musl port uses that interface for POSIX library calls. Rust
applications use the existing x86-64 musl standard library and link to
Lumen's libc sysroot through the toolchain wrappers.

## Build and boot the applications

On Debian or Ubuntu, install Clang, LLD, the Clang compiler-rt development
package, GNU cpio, patch, and Freedoom in addition to the
[kernel dependencies](build.md). On Ubuntu the package names are `clang`,
`lld`, `libclang-rt-dev`, `cpio`, `patch`, and `freedoom`.

Install the Rust targets and clone the upstream port sources alongside Lumen.

```sh
rustup target add x86_64-unknown-none x86_64-unknown-linux-musl
git clone --depth 1 --branch v1.2.5 https://git.musl-libc.org/git/musl ../musl
git clone --branch version-3.50.4 https://github.com/sqlite/sqlite.git ../sqlite
git clone https://github.com/ozkl/doomgeneric.git ../doomgeneric
git -C ../doomgeneric checkout e257a09527062bfb986b6e5e0780839b6cbd5b71
make -j8 all ports
make run-ports
```

`make ports` creates `build/ports/lumen.cpio`. QEMU supplies it through
fw_cfg, and Lumen extracts it to RAMFS during boot. `SQLITE_DIR`, `DOOM_DIR`,
and `DOOM_IWAD` override the checkout and WAD locations. `make run` boots the
base image with NVMe and Ethernet. The four-CPU QEMU machine uses Standard
VGA, RTL8139 and a 256 MiB RAM default.

At the `Lumen >` prompt in the QEMU window:

```text
run hello-std "two words"
run sqlite3 /demo.db "create table demo (n); insert into demo values (42);"
run sqlite3 /demo.db "select n from demo;"
run doom
run lumdesk
```

SQLite uses the dotfile-lock VFS with rollback journals. Databases and Doom
saves are stored in RAMFS for the current boot. Live NVMe/FAT32 storage is
accessible through the existing `fat` utility. Doom uses the Freedoom Phase 1
IWAD from the host package, a 640×400 framebuffer, and PS/2 key events.
Arrow keys move, Ctrl fires, Space uses doors, and Escape opens the menu.
Audio is disabled.

Use `make run-desktop` for the [login-gated desktop](desktop.md).

## Build another application

Build the sysroot once with `make libc`.

```sh
tools/lumen-rustc -C opt-level=s -C strip=symbols hello.rs -o hello
tools/lumen-cargo build --manifest-path /path/to/Cargo.toml --release
tools/lumen-clang -O2 -s hello.c -o hello
readelf -l hello
```

Both wrappers link the Lumen musl port built from musl v1.2.5 in
`build/libc/sysroot`. Set `LUMEN_SYSROOT` to use a different installation
or `CLANG` to select a compiler. The Rust wrapper takes its standard
library and unwinder from `x86_64-unknown-linux-musl` and selects Lumen's
libc and startup objects at link time.

Place applications in `bin/` in a `newc` cpio archive and pass it to the
base VM with `-fw_cfg name=opt/lumen/initrd,file=/path/to/archive.cpio`.
Archive names are relative to the RAMFS root, with directories before the
files they contain. The shell runs executables from `/bin`.

## Executable ABI

* The loader accepts little-endian x86-64 static `ET_EXEC` images. The
  wrappers link at `0x50000000`. Load segments can occupy the intervals
  `0x400000..0x800000` or `0x40000000..0xc0000000`. Each image has a 64 MiB
  mapped-size limit.
* Each ELF program begins at its header entry point with a private address
  space, page permissions from `PT_LOAD`, zero-filled BSS, and a guarded 1 MiB
  stack. The initial stack supplies `argc`, `argv`, `HOME`, `PATH`, `TERM`,
  and auxiliary-vector entries for the musl runtime. The scheduler saves
  x87/SSE state and the FS base used by TLS.
* `int 0x80` selects the operation in `rax`, with arguments in `rdi`,
  `rsi`, `rdx`, `r10`, `r8`, and `r9`. Signed results return in `rax`.
  `sdk/abi.rs` defines Lumen's operation numbers and structures.
  The musl bridge translates its internal operations and POSIX structures
  in userspace.
* The native calls cover file handles and directories, anonymous mappings,
  TLS, time and entropy, IPv4 sockets, IPC, process management, and display input.
  This environment currently supports single-threaded static executables.
  Synchronous process faults return exit status 128 plus the signal number.
* The BSP schedules userspace. Init and several device services still use
  the native flat-image ABI. The ELF syscall path has its own pointer
  validation, address-space lifecycle and descriptor table.

## Network

Lumen uses smoltcp with the [Lumeth RTL8139 driver](https://github.com/amogus-gggy/Lumeth).
The QEMU user network supplies the gateway at `10.0.2.2`. Lumen configures
the guest statically at `10.0.2.15/24`. IPv4 TCP client and listener sockets
and UDP datagrams are supported. Ports can use Rust `std::net` with numeric
IPv4 socket addresses. To expose a guest listener on host loopback:

```sh
make run-ports QEMU_NET='-nic user,model=rtl8139,hostfwd=tcp:127.0.0.1:8080-:8080'
```
