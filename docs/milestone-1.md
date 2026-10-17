# Elvaris Milestone 1: x86-64 Kernel Boot

## First Development Version Statement

> Elvaris v0.1 will boot in QEMU on x86-64, enter the kernel, print diagnostic output, and shut down safely.

## Target

```text
Architecture: x86-64
Machine: QEMU virtual machine
Interface: serial console
Kernel language: Rust
Assembly: only where required
First result: boot and print diagnostic text
```

ARM64 is an explicit second target and is not forgotten; it is deferred to Milestone 2 to keep early debugging tractable.

## Boot Approach

Use an existing bootloader initially. Writing a custom bootloader is a later, optional goal.

## First Kernel Boundary (allowed in Milestone 1)

- Kernel entry
- Early logging
- Architecture identification
- Panic handling
- Basic halt or shutdown behavior

## Explicitly Out of Scope for Milestone 1

- Networking
- Filesystems
- GUI
- Package management
- Windows compatibility
- Advanced scheduling
- Physical hardware support
- Complex user-space services

## Checklist

- [ ] Rust bare-metal target selected
- [ ] Boot method selected (existing bootloader)
- [ ] QEMU machine selected
- [ ] Kernel entry contract documented
- [ ] Linker strategy understood
- [ ] Serial output strategy understood
- [ ] Panic behavior defined
- [ ] Kernel builds without the host operating system
- [ ] Kernel image is produced
- [ ] Kernel boots in QEMU
- [ ] Diagnostic message appears
- [ ] QEMU exits or kernel halts safely
