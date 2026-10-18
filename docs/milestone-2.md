# Milestone 2: Early Kernel Foundation

## Objective

Establish reliable diagnostics and basic platform inspection before adding
any complex subsystems. The boot path proven in Milestone 1 must be able to
report its own failures clearly.

## Scope

- Serial output abstraction
- Kernel panic behavior
- BootInfo inspection and documentation
- Memory-map validation and reporting
- Testable QEMU termination
- First automated boot test

## Required behavior

1. All kernel output goes through a single serial abstraction, not scattered
   direct port writes.
2. The panic path prints a clear message including source location, then halts.
3. BootInfo is inspected and its fields are categorized.
4. A controlled memory-map summary is printed (count, usable total, reserved
   total, largest usable region) after validating the data.
5. QEMU can terminate in a way that distinguishes success, panic, unexpected
   halt, and timeout.
6. An automated test boots the kernel and checks the serial markers.

## BootInfo fields

Fields are categorized for the current `bootloader` 0.11.17 + UEFI
configuration.

### Required for Milestone 2

- `memory_regions` — validated and summarized.
- `kernel_addr` / `kernel_len` — physical location of the kernel ELF.
- `kernel_stack_bottom` / `kernel_stack_len` — stack provided by the bootloader.

### Optional for Milestone 2

- `framebuffer` — present under UEFI; recorded but not used for output.
- `rsdp_addr` — present under UEFI; recorded for later ACPI use.
- `api_version` — checked for compatibility.

### Deferred until a later milestone

- `physical_memory_offset` — only present if `map-physical-memory` is enabled.
- `recursive_index` — only present if `map-page-table-recursively` is enabled.
- `tls_template` — deferred until threads are supported.
- `ramdisk_addr` / `ramdisk_len` — deferred until filesystems are supported.
- `kernel_image_offset` — deferred until virtual-memory layout is finalized.

## Serial output design

- Single module: `kernel/src/serial.rs`.
- Functions: `init()`, `write_byte(byte)`, `write_str(slice)`, `write_newline()`,
  `write_fmt(args)`.
- COM1 at I/O port 0x3F8. The bootloader already initializes the UART, so
  `init()` verifies the port is reachable rather than reprogramming hardware.
- No buffering, no locking, no async. Output is synchronous and blocking.
- Newline is CRLF to match the serial console convention used in Milestone 1.

## Panic behavior

Decision: use Rust's `#[panic_handler]` with a custom implementation.

- The handler writes a panic message to serial, including the panic info
  (message and source location where available).
- It then halts the CPU in a loop with interrupts disabled.
- This keeps the failure path consistent with the existing handler and avoids
  adding a separate early-panic function that would need its own maintenance.

## Memory-map validation

Bootloader data is external input and must be validated before use.

Checks:
- Region count is within a sane bound.
- Each region's `start < end`.
- Regions do not overlap (report if they do, do not merge them silently).
- Only `Usable` regions are summed for usable totals.
- Integer overflow is avoided when accumulating totals (use checked/saturating
  arithmetic and reject overflow).

Reported summary:
- Memory region count
- Usable memory total (bytes)
- Reserved memory total (bytes)
- Largest usable region (start, end, size)

## QEMU test behavior

A development-only exit mechanism lets automated tests distinguish outcomes.

- Use the QEMU `isa-debugcon` / debug-exit device: the kernel writes a status
  code to a debug port, and QEMU exits with that code.
- Success: exit code 0.
- Panic: non-zero exit code.
- Unexpected halt: QEMU timeout kills the process (non-zero).
- This mechanism is development-only and is NOT part of the security model.

## Out of scope

- Heap allocation
- Page-frame allocation
- User mode
- Processes
- Scheduling
- Filesystems
- Networking
- Drivers beyond early serial output
- ARM64 implementation
- Graphics

## Exit criteria

- [x] Serial output is isolated in one module.
- [x] Panic path prints source location and halts.
- [x] BootInfo fields are documented and categorized.
- [x] Memory map is validated and summarized.
- [x] QEMU exits with a distinguishable status code.
- [x] An automated boot test passes and fails correctly.
- [x] All changes are committed in small, separate commits.