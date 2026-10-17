# Elvaris Boot Contract

Provisional document defining the agreement between the bootloader and the kernel for Milestone 1 (x86-64, QEMU, UEFI via Rust bootloader crate).

## 1. Kernel Entry

- Control is transferred by the bootloader jumping to the kernel entry point.
- CPU mode: 64-bit long mode (x86-64).
- Interrupts: disabled on entry (provisional).
- Stack: a valid stack provided by the bootloader, sized adequately for early kernel code.
- Paging: enabled by the bootloader (identity or simple mapping); kernel must not assume a particular higher-half layout.
- Processor state the kernel may rely on: standard x86-64 reset/UEFI handoff state as documented by the bootloader crate; CPUID available.

## 2. Memory Information

The kernel receives a memory map describing:

- Usable memory regions
- Reserved memory
- Firmware-reserved regions
- Bootloader memory
- Kernel memory
- Framebuffer memory (if provided)

Alignment requirements follow the bootloader crate's documented boot info. The kernel must never assume all physical memory is available.

## 3. Kernel Location

- The kernel is loaded by the bootloader at a location it chooses.
- Physical location may vary; kernel must use provided load info rather than hard-coded addresses.
- Mapping is whatever the bootloader establishes; for Milestone 1 keep it simple (no sophisticated higher-half design).
- Regions: executable for code, read-only for rodata, writable for data/bss/stack.

## 4. Console Information

- Primary console for Milestone 1: serial port (COM1, 115200 baud, 8N1) (provisional).
- Framebuffer may be provided by the bootloader but is not required.
- If console initialization fails: print a diagnostic if possible, then halt safely.

## 5. Boot Arguments

- Kernel command-line arguments, debug flags, and logging level are optional and may be unused initially.

## 6. Initial Modules

No initial user-space modules are required for Milestone 1.

## 7. Failure Behavior

On invalid memory info, incomplete boot info, unsupported environment, or fatal early error: print a diagnostic message if possible, then halt safely.

## Minimal Contract Summary

- Architecture identity
- Memory map
- Kernel load information
- Early console information
- Optional boot arguments
- Bootloader version or identifier
