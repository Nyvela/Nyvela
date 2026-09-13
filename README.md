# Nyvela

A modern 64-bit operating system written from scratch in x86-64 Assembly and C.

Nyvela is an experimental operating system focused on understanding and implementing the fundamentals of a computer system, from the boot process and CPU initialization to memory management, hardware support, and userspace.

## Goals

The goal of Nyvela is to build a fully usable operating system from scratch, without relying on prebuilt operating system components or third-party libraries for core functionality. It pursues a microkernel architecture for stability, keeping the privileged kernel minimal and isolating drivers, filesystems, and other core services in userspace.

The project aims to provide full control over the system, including:

* Boot and CPU initialization
* Memory management
* Interrupt handling
* Hardware communication
* Filesystems
* Process and thread management
* Userspace programs
* A graphical environment

## Roadmap

### Boot

* [x] Boot from disk
* [x] Enable protected mode
* [x] Enable A20
* [x] Set up IDT
* [x] Enable long mode
* [x] Set up paging
* [x] Load kernel

### Kernel

* [x] Initialize hardware (LAPIC + LAPIC timer)
* [x] Implement physical memory management (`kpmm_init`, `kpalloc`/`kpfree`)
* [x] Implement virtual memory (`kvmm_init`, `kvmmap`/`kvmunmap`)
* [x] Implement interrupts and exceptions (IDT + DE/PF/GP ISRs + LAPIC timer tick)
* [x] Implement a heap allocator (`kmalloc`/`krealloc`/`kfree`)
* [x] Implement basic threads (`spawn_thread`; no processes yet)
* [x] Implement a basic scheduler (round-robin on LAPIC timer)
* [ ] Implement system calls
* [x] Add x86-64 asm abstractions (`arch/x86_64/asm/`: cpu/io/desc/msr/cpuid)

### Userspace

* [ ] Implement userspace support
* [ ] Implement a standard library
* [ ] Implement a shell
* [ ] Implement basic system utilities
* [ ] Implement filesystem support

### Graphics

* [ ] Implement framebuffer support
* [x] Implement text rendering (VGA text-mode console; no framebuffer fonts yet)
* [ ] Implement input handling
* [ ] Build a graphical interface

### Technical debt

* [ ] Rewrite everything to Rust

## License

GPL-3.0
