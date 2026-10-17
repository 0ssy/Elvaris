# Elvaris Development Environment

## 1. Purpose

This document records the bare-metal development environment required to build and test Elvaris. It is intended to make the project reproducible on a fresh machine.

Status: **Provisional.** Tool versions and installation commands will be updated once the environment is fully verified.

## 2. Development Host

- Operating system: Windows 10.0.26300 (win32) — used for repository work.
- Recommended build environment for bare-metal Rust: WSL2 (Ubuntu) or a Linux VM, because the Rust bare-metal toolchain, GNU binutils, and QEMU are most readily available there. Windows-native paths are possible but not yet verified.
- Provisional decision: use WSL2 (Ubuntu) as the primary build/test host. (Marked provisional.)

## 3. Required Tools

| Tool | Purpose | Status |
|------|---------|--------|
| Git | Version control | Installed (2.55.0.windows.3) |
| Rust toolchain (rustc, cargo, rustup) | Kernel language | Not installed |
| Rust source components | `rust-src` for bare-metal builds | Not installed |
| x86-64 target support | `x86_64-unknown-none` (provisional triple) | Not installed |
| ARM64 target support | `aarch64-unknown-none` (provisional triple) | Not installed |
| Linker | `ld` (via GNU binutils or lld) | Not installed (clang 21.1.6 present) |
| Assembler | `as` (GNU binutils) | Not installed |
| QEMU x86-64 | Emulator for first target | Not installed |
| QEMU ARM64 | Emulator for second target | Not installed |
| GDB | Debugger | Not installed |
| Build utilities | make/cmake/ninja (as needed) | Not verified |

## 4. Required Target Triples (provisional)

- x86-64: `x86_64-unknown-none` (provisional)
- ARM64: `aarch64-unknown-none` (provisional)

Final triples will be confirmed against the chosen bootloader's expectations.

## 5. Installation Commands (provisional, Linux/WSL)

```sh
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add rust-src
rustup target add x86_64-unknown-none
rustup target add aarch64-unknown-none

# GNU binutils + QEMU + GDB
sudo apt update
sudo apt install -y build-essential qemu-system-x86 qemu-system-arm gdb
```

## 6. Verification Procedure

For each tool, run the version command and record the output here before proceeding to kernel code:

- [ ] `rustc --version`
- [ ] `cargo --version`
- [ ] `rustup --version`
- [ ] `rustup target list --installed` (must include both triples)
- [ ] `qemu-system-x86_64 --version`
- [ ] `qemu-system-aarch64 --version`
- [ ] `gdb --version`
- [ ] `as --version`
- [ ] `ld --version`
- [ ] QEMU starts a trivial test image without error

## 7. Known Problems

- On the current Windows host, Rust, GNU binutils, QEMU, and GDB are not on PATH. Only Git and clang are available.
- Provisional: perform all bare-metal builds inside WSL2 until a native Windows toolchain is verified.
- Bootloader choice, exact target triples, and linker script details remain provisional until Milestone 1 is implemented.

## 9. Bootloader Decision (provisional)

```text
Initial bootloader: Rust bootloader crate
Initial architecture: x86-64
Initial firmware path: UEFI
Initial test environment: QEMU
Initial kernel output: serial console
ARM64 boot path: deferred until Milestone 2
Custom bootloader: not part of the initial milestone
```

## 11. Expanded Tool Checklist

Required tools for Milestone 1:

| Tool | Purpose | Status |
|---|---|---|
| Rust toolchain | Kernel and bootloader development | MISSING |
| rust-src | Bare-metal Rust target support | MISSING |
| llvm-tools-preview | Linker/binary utilities for Rust OS workflows | MISSING |
| x86-64 bare-metal target | Kernel compilation target | MISSING |
| QEMU x86-64 | Virtual machine testing | MISSING |
| OVMF | UEFI firmware for QEMU | MISSING |
| GDB | Kernel debugging | MISSING |
| NASM (or another assembler) | Early architecture-specific work | MISSING |
| Build utilities | Packaging and automation | NOT VERIFIED |

Already present: Git 2.55.0.windows.3, Clang 21.1.6.

## 12. Setup Order (provisional, host-appropriate)

1. Install Rust via rustup.
2. `rustup component add rust-src`
3. `rustup component add llvm-tools-preview`
4. `rustup target add x86_64-unknown-none`
5. Install QEMU x86-64.
6. Install/locate OVMF UEFI firmware.
7. Install GDB.
8. Install NASM (or equivalent assembler).
9. Record every installed version in this document.

Use the method appropriate for the host OS; do not blindly use Linux commands on Windows/macOS.

## 13. Verification Checklist

```text
[ ] rustc runs
[ ] cargo runs
[ ] rustup runs
[ ] rust-src is installed
[ ] llvm-tools-preview is installed
[ ] x86-64 bare-metal target is installed
[ ] QEMU x86-64 runs
[ ] UEFI firmware is available
[ ] GDB runs
[ ] Assembler runs
[ ] All versions recorded
```

Confirm: QEMU can start an x86-64 VM; UEFI firmware file locatable; Rust recognizes the bare-metal target; bootloader crate requirements compatible with installed Rust version.

## 14. Pinned Versions (to be filled after install)

```text
Rust version:
Cargo version:
QEMU version:
GDB version:
Clang version: 21.1.6
Assembler version:
LLVM version:
OVMF firmware path:
Host operating system: Windows 10.0.26300
```


Verified on host (Windows 10.0.26300). Rust, binutils, QEMU, GDB, OVMF not installed; only Git and clang present.

- Git: installed — 2.55.0.windows.3
- Rust toolchain: MISSING (rustc/cargo/rustup not on PATH)
- Rust source component: MISSING
- x86-64 bare-metal target: MISSING
- Linker (ld/as): MISSING (clang 21.1.6 present but GNU binutils absent)
- Assembler: MISSING
- QEMU x86-64: MISSING
- QEMU ARM64: MISSING
- UEFI firmware (OVMF): MISSING / not verified
- GDB: MISSING
- Build utilities: not verified

These must be installed (provisional plan: inside WSL2 Ubuntu) before kernel work begins. Plan is not silently changed; missing tools are recorded as missing.


- First target: x86-64 QEMU (provisional until verified, but recommended and adopted).
- Initial boot method: existing bootloader (adopted). Writing a custom bootloader is deferred.
- Exact bootloader (e.g. bootloader crate, GRUB, or another) is the next decision to be documented.
