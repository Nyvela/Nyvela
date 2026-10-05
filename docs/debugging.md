# Debugging

## make debug

```sh
make debug
```

Boots QEMU paused with a GDB stub:

```sh
qemu-system-x86_64 -drive format=raw,file=build/os.img \
  -d int,cpu_reset,guest_errors -D qemu.log \
  -no-reboot -no-shutdown -s -S
```

Then:

```sh
gdb build/kernel.elf
(gdb) target remote :1234
(gdb) break kmain
(gdb) continue
```

`qemu.log` holds interrupt traces and CPU resets. `-no-reboot` matters when
debugging boot problems: a triple fault resets the machine, and without that
flag the guest would silently reboot and you would lose the failure.

## Reading the screen without a display

`make run` opens a window. To capture the console from a headless run:

```sh
qemu-system-x86_64 -drive format=raw,file=build/os.img \
  -display none -no-reboot -no-shutdown \
  -monitor unix:/tmp/mon.sock,server,nowait &
sleep 8
printf 'pmemsave 0xb8000 0x5000 "/tmp/vga.bin"\n' | socat - UNIX-CONNECT:/tmp/mon.sock
```

`0xb8000` is the VGA text buffer, 160 bytes per row, 80x25. Dump it as text to
see what the kernel printed, or `x/` it in the monitor to inspect registers and
memory at the moment it stalled.

## Verifying the kernel handoff

The most useful boot check is confirming that stage 1 and stage 2 actually
delivered `kernel.bin` intact. Break at `kmain` and compare the staged buffer
against the image (substitute the sector count `make` printed for `N`):

```sh
(gdb) break kmain
(gdb) continue
(gdb) dump binary memory /tmp/staged.bin 0x10000 0x10000+N*512
$ cmp /tmp/staged.bin build/kernel.bin && echo "kernel image intact"
```

To check the relocated copy at `0x100000`, dump from there instead. A mismatch
starting at `0x10000` means stage 1's chunk bookkeeping is wrong; a correct
`0x10000` but wrong `0x100000` means stage 2's `KERNEL_SIZE_IN_SECTORS` does not
match the read.

## Tracing a boot-stage handoff

All three loaders are small enough to set breakpoints on directly:

```sh
(gdb) break *0x7c00     ; boot.s entry
(gdb) break *0x8000     ; stage_1 entry
(gdb) break *0x808a     ; stage_1, just after the kernel read loop
(gdb) break *0x8800     ; stage_2 entry
```

Breakpoint addresses shift when `stage_1.s` changes, since they are offsets into
the generated binary - re-derive them with
`ndisasm -b16 -o 0x8000 build/stage_1.bin`.

To see the disk address packet contents:

```sh
(gdb) x/2gx 0x869d
```

The DAP is 16 bytes: size, reserved, sector count, offset, segment, and a 64-bit
LBA.

## Troubleshooting

| symptom | likely cause |
|---------|--------------|
| `[ FERR ] Failed to load Kernel to memory.` then halt | the read failed. |
| kernel runs but prints nothing, RIP inside `klog_ram_data` | memory at `0x100000` is not `kernel.bin`. Check `KERNEL_SIZE_IN_SECTORS` in `stage_2.s` and the chunk loop in `stage_1.s` |
| ring3 `#GP` a couple of bytes into `_start` | the flat binary does not start with code, the linker script was not passed with `-T`, so offset 0 is a GNU note. See [build system](build.md) |
| triple fault, machine reboots, `qemu.log` shows `CPU Reset` | usually a bad `iretq` frame in `switch_context`, a `TSS.rsp0` that does not match the scheduled thread, or a GDT descriptor with the wrong limit/DPL |
| `make` stops with `<KEY> is not configured` | a required `nyvela.conf` key is missing - any of the six (`INIT_DIR`, `INIT_BIN_NAME`, `INIT_BUILD_COMMAND`, `SHELL_DIR`, `SHELL_BIN_NAME`, `SHELL_BUILD_COMMAND`) |
| `make` stops with `init directory ... does not exist` | `INIT_DIR` points somewhere that is not there; check the relative path in `nyvela.conf` |
| init build fails with `nyvela/user/syslib.h: No such file` | `INIT_BUILD_COMMAND` is not forwarding `USER_CFLAGS` |

## Fault handler output

Ring3 and ring0 faults land in the same log-and-halt handlers. The dump gives
you the vector, error code, `RIP`/`CS`/`RSP`, and 16 bytes at `RIP` in hex - the
raw bytes are usually enough to identify the faulting instruction immediately.
Remember a `#GP` error code of `0..4`-ish in the low bits usually encodes a
*segment selector* (`bits 15:3` index, `bit 2` `TI`, `bits 1:0` `RPL`) rather
than an address, in which case `CR2` is meaningless.

There is no recovery path yet, so any fault halts the machine. Adding recovery
means giving the IDT an IST so the handler has a stack of its own.

## See also

[Boot chain](boot.md), [userspace](userspace.md).
