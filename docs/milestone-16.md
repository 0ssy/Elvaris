# Milestone 16: ARM64 Port

## Objective

Bring the existing Elvaris operating-system core to AArch64/ARM64, initially running in QEMU. The port reuses architecture-independent components (process model, scheduler, kernel heap, filesystem, program loader, shell, security model, system-call definitions) and provides ARM64 implementations for boot, CPU initialization, exceptions, virtual memory, interrupts, timers, context switching, user-mode transitions, system-call entry, and console/storage devices.

## Target Platform

- Machine: QEMU `virt`
- Architecture: AArch64
- Kernel target: `aarch64-unknown-none`
- Firmware: none required; QEMU loads the kernel ELF directly with `-kernel` (boot at EL2, `x0` holds the device-tree pointer)
- Console: ARM64 PL011-compatible serial device at `0x09000000`
- Storage: QEMU virtio-mmio virtual block device (optional; the initial filesystem runs in memory via MemFs)
- Interrupt controller: ARM GICv2
- Timer: ARM generic timer (physical timer at EL1)

## Reused Components

- Process model and thread state model
- Scheduler policy
- Kernel heap allocator
- Filesystem interfaces and in-memory filesystem (MemFs)
- Program-loader logic
- Shell and terminal line discipline
- Security identities, authorization rules, and process cleanup logic
- System-call numbers, logical arguments, return-value semantics, error model
- Network protocol code (unit-tested, fake device only)
- Error types where architecture-independent
- Diagnostic markers (`arch: aarch64`, `boot: ready`, `pmm: ready`, ...)

## ARM64-Specific Components

All located under `arch/aarch64/`, never spread as conditionals through generic code:

- Boot entry (`arch/aarch64/boot.rs`)
- CPU initialization and exception levels (`arch/aarch64/cpu.rs`)
- Exception vectors (`arch/aarch64/exceptions.rs`)
- Page tables, address translation, TLB maintenance (`arch/aarch64/memory.rs`)
- Physical memory map parsing from the device tree (`arch/aarch64/pmm.rs`)
- GICv2 interrupt controller (`arch/aarch64/cpu.rs`)
- ARM generic timer (`arch/aarch64/cpu.rs`)
- Context switching (`arch/aarch64/switch.S`)
- User-mode entry and system-call entry (`arch/aarch64/syscall.rs`)
- PL011 serial console (`arch/aarch64/console.rs`)
- Cache and TLB maintenance, atomic memory ordering (`arch/aarch64/`)

## Boot Strategy

QEMU `virt` boots CPUs at EL2 and jumps to the kernel entry passed via `-kernel`. `x0` = physical address of the device tree blob. The boot path:

1. QEMU loads the kernel ELF at its link address and enters at EL2 (hyp mode), interrupts masked, stack not yet set up.
2. The boot entry sets up a minimal stack, reads the device tree, extracts the usable memory region and the serial/base addresses, and fills the Elvaris boot-information structure.
3. The entry switches to EL1h, installs the stage-1 translation table, enables the MMU, and jumps to the generic `kernel_main(boot_info)`.
4. The generic kernel receives the same boot contract on x86-64 and ARM64: memory regions, kernel location/size, initial stack, console information, hardware-description information, firmware identifier.

Exit: the kernel halts in a `wfi` loop after printing the `shutdown: ready` marker; the test harness terminates QEMU deterministically once that marker appears (QMP `quit`), so the run ends on a stable condition rather than a timeout.

## Exception Levels

- EL2: QEMU entry point; used only to configure the system and hand control to EL1.
- EL1h: kernel execution (supervisor), stage-1 MMU and timer at this level.
- EL0: user programs.
- EL3: not used (no TrustZone, no secure firmware).

## Memory Model

- Physical-address width: 48 bits (QEMU `virt` with `highmem=off`, up to 1 GiB RAM by default; the port reads the actual size from the device tree).
- Virtual-address width: 48 bits.
- Direct physical-memory map: not assumed; page tables are accessed through identity mappings or a fixed kernel virtual offset chosen at boot (`0xFFFF_0000_0000_0000` is avoided; the kernel maps itself at a fixed high range established by the boot entry).
- Memory types: Normal (cacheable, write-back) for RAM, Device-nGnRnE for device registers, with outer/shareable attributes as appropriate.

## Page Tables

- Translation granule: 4 KiB.
- Virtual-address width: 48 bits, 4 translation levels (L0..L3), 9 bits per level, 512 entries per table.
- Block mappings: supported for large contiguous regions at boot (2 MiB sections), 4 KiB pages afterward.
- Descriptors: table descriptors (valid, next-level), page descriptors (AP, XN/UXN, AF, SH, memory attributes).
- Access permissions: AP[2:1] = 00 supervisor RW, 01 supervisor RO, 10 user RW, 11 user RO.
- Execute-never: XN bit (bit 54) for supervisor, PXN/UXN for user.
- Shareability: Inner Shareable for kernel and user RAM.
- Memory attributes: Normal Inner/Outer Write-Back Read-Allocate Write-Allocate; Device-nGnRnE for MMIO.
- TLB invalidation: `tlbi vmalle1is` after permission/table changes, followed by `dsb ish` and `isb`.
- ASIDs: not used initially (full TLB invalidate on address-space switch).

## Interrupt Controller

- ARM Generic Interrupt Controller, version 2 (GICv2), configured explicitly with `machine=virt,gic-version=2`.
- Distributor at `0x08000000`, CPU interface at `0x08010000` (QEMU `virt` GICv2 addresses).
- Required interrupts: generic timer interrupt (IRQ 27, virtual/physical timer), PL011 receive interrupt if interrupt-driven input is used.
- Minimum implementation: enable distributor and CPU interface, set priority/mask, acknowledge (ICCIAR), end-of-interrupt (ICCEOIR), unexpected-interrupt diagnostics.

## Timer

- ARM generic timer, physical timer at EL1 (`CNTP_CTL_EL1`, `CNTP_TVAL_EL1`, `CNTP_CVAL_EL1`, timer interrupt IRQ 27).
- Frequency read from `CNTFRQ_EL0`.
- Tick counting drives scheduler preemption (100 Hz, shared with x86-64).

## Context Switching

- Saved state: general-purpose registers X19-X28 (callee-saved), stack pointer, link register, saved program status / exception level state.
- Floating-point/SIMD state: not saved (Elvaris does not use it at this stage; documented limitation).
- Kernel threads switch cooperatively (`yield`) and preemptively (timer interrupt).

## User Mode

- User exception level: EL0, with SP_EL0 for the user stack.
- Kernel exception return: `eret` to EL0 with `SPSR` set to EL0t (64-bit, SP_EL0, interrupts masked/unmasked as configured).
- System-call instruction: `SVC #0`.
- System-call arguments/return: `x0`-`x7` per the ARM64 calling convention; system-call number in `x8`, result in `x0`.
- User stack: non-executable; user code page: executable and read-only; user data: read/write, non-executable.

## System Calls

Shared system-call numbers and semantics with x86-64; only the entry/return mechanism is ARM64-specific:
- `SVC #0` from EL0 traps to EL1.
- The exception handler saves the user frame, decodes the number in `x8`, dispatches to the generic handler, restores the frame, and returns with `eret`.

## Console

- PL011 UART at `0x09000000` (QEMU `virt`), 115200 baud, 8N1.
- Architecture-specific serial backend behind the existing generic `Console`/`InputSource`/`Output` abstraction.
- Initialization, byte transmission, string output, newline output, optional byte reception, device error handling, debug-only verbose output.
- Generic kernel uses the same console abstraction on both architectures.

## Storage

- Initial ARM64 filesystems run in memory (MemFs): no block device required for `init` and the shell.
- Where a block device is used, an ARM64-specific virtio-mmio backend provides device discovery, block reads/writes, reset, errors, and shutdown behind the generic block-device interface.

## Security

- The security policy is identical to x86-64: user/group identities, credential inheritance, file ownership, file permissions, process permissions, protected memory, protected devices, restricted shutdown, security logging.
- Verified: user pages cannot access kernel pages; kernel pages have correct execute permissions; user stack is non-executable; invalid user pointers are rejected; exception handlers validate origin and privilege; device mappings are not user-accessible; TLB invalidation occurs after permission changes; context switches cannot leak register state between processes; kernel stacks are not exposed to user mode.

## Testing Strategy

- Build tests: generic crates build for `aarch64-unknown-none`; ARM64 kernel builds; x86-64 build remains green.
- Unit tests: filesystem, process states, security, shell parsing, packet parsing (host target); ARM64-specific (page-table flags, exception decoding, register-frame handling, timer calculations, context layout, address translation).
- QEMU integration tests: boot, serial output, PMM, virtual memory, heap, exceptions, timer, kernel threads, preemptive scheduling, user mode, system calls, process exit, process cleanup, filesystem, init, shell, terminal input, security, shutdown.
- Cross-architecture regression: every ARM64 change still passes x86-64; every architecture-independent change is tested on both targets.

## Diagnostics

Architecture-neutral markers plus ARM64-specific markers:
- `arch: aarch64`, `boot: ready`, `pmm: ready`, `vmm: ready`, `interrupts: ready`, `timer: ready`, `scheduler: ready`, `user: ready`, `process: ready`, `filesystem: ready`, `init: ready`, `shell: ready`, `security: ready`, `shutdown: ready`
- `arm64: exception level`, `arm64: translation granule`, `arm64: virtual address width`, `arm64: physical address width`, `arm64: timer frequency`, `arm64: interrupt controller version`, `arm64: serial device`

## Out of Scope

Physical ARM64 hardware, SMP, NUMA, TrustZone, SVE, KVM/hypervisor, 32-bit ARM, Raspberry Pi, Wi-Fi, USB, GPU, device-tree support for every board, new compatibility layers.

## Exit Criteria

- ARM64 toolchain and QEMU target documented
- ARM64 boot path works; boot contract documented
- ARM64 serial console works
- ARM64 memory map parsed; PMM works
- ARM64 page tables, mappings, permissions work
- ARM64 exceptions, interrupt controller, timer work
- ARM64 kernel threads, preemptive scheduling work
- ARM64 user mode, system calls, process cleanup work
- ARM64 storage, filesystem, init, shell, terminal input work
- ARM64 security behavior matches the documented model
- Architecture-independent tests pass on ARM64; ARM64 integration tests pass
- All existing x86-64 tests still pass
- ARM64 limitations documented

## Definition of Done

Boot ARM64 Elvaris in QEMU → reach the kernel entry → print architecture and boot diagnostics → initialize PMM and virtual memory → initialize interrupts and timer → start the scheduler → enter ARM64 user mode → run a user program → perform a system call → exit and reap the user process → mount the filesystem → start init → start the shell → accept terminal input → load and execute a program from the filesystem → enforce permissions → shut down cleanly.
