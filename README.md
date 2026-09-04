# Nyvela

A modern 64-bit operating system written from scratch in x86-64 Assembly and C.

Nyvela is an experimental operating system focused on understanding and implementing the fundamentals of a computer system, from the boot process and CPU initialization to memory management, hardware support, and userspace.

## Goals

The goal of Nyvela is to build a fully usable operating system from scratch, without relying on prebuilt operating system components or third-party libraries for core functionality.

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
* [ ] Set up IDT
* [ ] Enable long mode
* [ ] Set up paging
* [ ] Load kernel

### Kernel

* [ ] Initialize hardware
* [ ] Implement physical memory management
* [ ] Implement virtual memory
* [ ] Implement interrupts and exceptions
* [ ] Implement a heap allocator
* [ ] Implement processes and threads
* [ ] Implement a scheduler
* [ ] Implement system calls

### Userspace

* [ ] Implement userspace support
* [ ] Implement a standard library
* [ ] Implement a shell
* [ ] Implement basic system utilities
* [ ] Implement filesystem support

### Graphics

* [ ] Implement framebuffer support
* [ ] Implement text rendering
* [ ] Implement input handling
* [ ] Build a graphical interface

## License

GPL-3.0
