# Elvaris Development Environment

## 1. Purpose

This document records the development environment used to build and test Elvaris.
It exists to make the project reproducible.

## 2. Host System

- Operating system used for development: Windows (host triple `x86_64-pc-windows-msvc`)
- Git version: 2.55.0.windows.3

## 3. Toolchain

- Rust toolchain: stable (`rustc 1.99.1`, `cargo 1.99.1`)
- Installed Rust targets:
  - `x86_64-unknown-none` (primary bare-metal target, provisional)
  - `aarch64-unknown-none` (ARM64 bare-metal target, deferred)
  - `x86_64-pc-windows-msvc` (host)

## 4. Required Tools (status)

| Tool | Required | Installed | Version | Notes |
|------|----------|-----------|---------|-------|
| Git | yes | yes | 2.55.0 | |
| Rust / cargo | yes | yes | 1.99.1 | |
| rustup | yes | yes | - | |
| x86-64 bare-metal target | yes | yes | `x86_64-unknown-none` | |
| ARM64 bare-metal target | yes (deferred) | yes | `aarch64-unknown-none` | |
| Linker (ld) | yes | **no** | - | To be installed (e.g. via LLVM/MinGW) |
| Assembler (as) | yes | **no** | - | To be installed |
| QEMU x86-64 | yes | **no** | - | To be installed |
| QEMU ARM64 | yes (deferred) | **no** | - | To be installed |
| GDB | yes | **no** | - | To be installed |

## 5. First Target (frozen, provisional where noted)

- Architecture: x86-64
- Machine: QEMU virtual machine
- Interface: serial console
- Kernel language: Rust
- Assembly: only where required
- First result: boot and print diagnostic text
- ARM64: explicit second target (provisional timeline)

## 6. Boot Method (frozen)

- Initial boot method: existing bootloader (provisional choice; exact bootloader TBD)
- Rationale: faster first milestone, less early assembly, easier memory-map handling.

## 7. Installation Commands (to be completed)

- Install QEMU for Windows and add to PATH.
- Install a linker/assembler toolchain (LLVM or MinGW) and add to PATH.
- Install GDB (or use rust-gdb).
- Verify with: `qemu-system-x86_64 --version`, `rustc --version`, `cargo --version`.

## 8. Known Problems

- QEMU, GDB, and binutils are not currently installed on the development machine.
- Linker strategy for bare-metal Rust on Windows host is not yet finalized (provisional).

## 9. Verification

A tool is considered verified when its `--version` (or equivalent) runs successfully and the bare-metal Rust target compiles a `#![no_std]` crate without host OS dependencies.
