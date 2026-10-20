# Milestone 6: CPU, Interrupts and Exceptions

## Objective

Bring up the kernel CPU infrastructure required to handle synchronous and asynchronous events: the global descriptor table and task state segment, the interrupt descriptor table with exception handlers, legacy interrupt controllers, the local APIC, and a kernel timer. Prove that interrupts work in QEMU and that the CPU state is observable through serial diagnostics.

## Scope

- GDT with kernel/user code/data segments and a TSS descriptor
- TSS with ring 0 stack and dedicated double-fault (IST) stack
- IDT with handlers for all CPU exceptions and hardware interrupts
- Exception handling with correct 64-bit interrupt frame layout and error-code detection
- PIC remapping and APIC initialization
- PIT/APIC timer support producing periodic interrupts
- CPU state reporting
- Documentation of bootloader-provided CPU state and CPU initialization

## Key Implementation Details

### GDT/TSS
- Flat segments covering required privilege levels
- TSS descriptor encoded correctly for 64-bit mode (type field at bit 40, base[23:16] masked with 0xFF)
- Segment registers reloaded with full iretq frame (SS, RSP, RFLAGS, CS, RIP)
- Task register loaded after GDT installation

### IDT/Exception Handling
- 64-bit interrupt frame: r15..rax, vector, error_code, rip, cs, rflags, rsp, ss (176 bytes total)
- Error-code vectors: 8, 10, 11, 12, 13, 14, 17, 21
- User-mode detection via CS/RPL
- Double faults routed to dedicated IST stack

### Interrupt Controllers
- PIC remapped to avoid vector collisions with exceptions
- Local APIC enabled with timer programmed for kernel timer frequency
- Timer interrupts verified to be delivered

## Verification

- Host unit tests pass for descriptor encoding, frame layout, and interrupt utilities
- QEMU boot reaches "segments reloaded", installs GDT/TSS/IDT, enables interrupts, and receives timer interrupts
- Integration tests pass (boot and heap)
