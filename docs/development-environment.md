# Elvaris Development Environment

## 1. Purpose

This document records the bare-metal development environment required to build and test Elvaris. It is intended to make the project reproducible on a fresh machine.

Status: **Verified.** All required tools are installed and verified on the host. Versions are pinned below.

## 2. Development Host

- Operating system: Windows 10.0.26300 (win32).
- Build host: Windows-native (not WSL2). All required installers are present or downloadable on Windows and have been verified.
- Target triple: `x86_64-unknown-none` (primary), `aarch64-unknown-none` (Milestone 2 parity).

## 3. Required Tools

| Tool | Purpose | Status |
|------|---------|--------|
| Git | Version control | Installed (2.55.0.windows.3) |
| Rust toolchain (rustc, cargo, rustup) | Kernel language | Installed (1.99.1) |
| rust-src | Bare-metal Rust source | Installed |
| llvm-tools-preview | Linker/binary utilities for OS workflows | Installed |
| x86-64 bare-metal target | Kernel compilation target | Installed (`x86_64-unknown-none`) |
| ARM64 bare-metal target | Milestone 2 parity | Installed (`aarch64-unknown-none`) |
| QEMU x86-64 | Emulator for first target | Installed (11.1.0) |
| OVMF / UEFI firmware | UEFI boot via QEMU | Installed (rust-osdev/ovmf-prebuilt, edk2-stable202605-r1) |
| GDB | Debugger | Installed (18.1) |
| NASM | Assembler | Installed (3.01) |
| Clang / LLVM | C linker and build utilities | Installed (21.1.6 Swift / 22.1.8 LLVM) |
| Build utilities | make/cmake/ninja (optional; Cargo handles most builds) | CMake present; Cargo primary |

## 4. Required Target Triples

- x86-64: `x86_64-unknown-none` (verified)
- ARM64: `aarch64-unknown-none` (verified, for Milestone 2 parity)

Final triples will be confirmed against the chosen bootloader's expectations in the follow-up review step.

## 5. Verification Checklist

```text
[x] rustc --version
[x] cargo --version
[x] rustup --version
[x] rustup component list --installed  (rust-src, llvm-tools-preview)
[x] rustup target list --installed  (x86_64-unknown-none, aarch64-unknown-none)
[x] qemu-system-x86_64 --version
[x] QEMU starts an x86-64 VM with OVMF firmware (UEFI shell reached; "No bootable option or device was found" confirms firmware executed)
[x] UEFI firmware file located and path recorded
[x] gdb --version
[x] nasm -v
[x] All versions recorded in this document
```

## 6. Pinned Versions

```text
Rust version: 1.99.1 (58cf8f9c3 2026-10-13)
Cargo version: 1.99.1 (5f94df478 2026-08-27)
rustup version: 1.29.1 (d95a37b6a 2026-08-13)
QEMU version: 11.1.0 (v11.1.0-12130-ge470268ff4)
GDB version: 18.1 (C:\msys64\mingw64\bin\gdb.exe)
Clang version: 21.1.6 (Swift toolchain, on PATH) / 22.1.8 (LLVM, C:\Program Files\LLVM\bin)
Assembler version: NASM 3.01 (compiled Oct 10 2025; C:\Users\josep\Desktop\Elvaris\nasm-3.01\nasm-3.01\nasm.exe)
LLVM version: 22.1.8 (C:\Program Files\LLVM)
OVMF firmware path: C:\Users\josep\Desktop\Elvaris\ovmf-x86_64\OVMF_CODE.fd (and OVMF_VARS.fd)
Host operating system: Windows 10.0.26300 (win32)
```

## 7. Notes

- QEMU is installed at `C:\Program Files\QEMU\qemu-system-x86_64.exe` and is on PATH via the `qemu` directory.
- The QEMU-bundled `edk2-x86_64-code.fd` is a build artifact and is rejected by QEMU ("could not load PC BIOS"). A proper OVMF build (rust-osdev/ovmf-prebuilt, tag `edk2-stable202605-r1`) is used instead and stored under `ovmf-x86_64/`.
- NASM is installed from the extracted `nasm-3.01/nasm-3.01/` directory and added to the user PATH.
- GDB 18.1 is installed via MSYS2 at `C:\msys64\mingw64\bin\gdb.exe`.
- Do not upgrade tools casually after implementation begins; all versions are pinned above.
- Bootloader choice, exact target triples, and linker script details remain provisional until Milestone 1 is implemented.