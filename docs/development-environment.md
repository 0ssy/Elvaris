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

## 8. Boot Approach Decision

- First target: x86-64 QEMU (provisional until verified, but recommended and adopted).
- Initial boot method: existing bootloader (adopted). Writing a custom bootloader is deferred.
- Exact bootloader (e.g. bootloader crate, GRUB, or another) is the next decision to be documented.
