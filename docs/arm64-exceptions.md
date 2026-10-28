# ARM64 Exceptions

## Exception categories and behavior

- Synchronous instruction abort: diagnostic + `EXIT_EXCEPTION`, halt
- Synchronous data abort: diagnostic (fault address, fault status, access type) + `EXIT_EXCEPTION`, halt; invalid user access isolates the user process instead of halting the machine
- Undefined instruction: diagnostic + `EXIT_EXCEPTION`, halt
- System-call exception (`SVC #0`): dispatch to the generic system-call handler, return to user via `eret`
- Breakpoint: diagnostic + halt
- Alignment fault: diagnostic + halt
- Permission fault: diagnostic + halt (or process termination when the fault came from user mode)
- Translation fault: diagnostic + halt
- External interrupt (IRQ): GICv2 acknowledge, dispatch, end-of-interrupt
- Timer interrupt: generic timer physical interrupt, scheduler tick

## Exception vector

- Location: a single vector table at the start of a read-only code page, referenced by `VBAR_EL1`
- Alignment: 2 KiB aligned (required for vectored exceptions)
- Current exception level: read from `CurrentEL`
- Register-save frame: saved general-purpose registers (`x0`-`x30`), stack pointer, `elr`, `spsr` on a per-exception frame
- Return instruction: `eret`
- Error-register capture: `ESR_EL1`, `FAR_EL1`, `ELR_EL1`
- Exception-class decoding: `ESR_EL1.EC` field distinguishes instruction abort, data abort, undefined instruction, SVC, etc.

## Entry from user mode

- `SPSR` selects EL0t; `SP_EL0` is the user stack pointer on entry
- On return to user mode: `eret` restores `ELR_EL0` (program counter) and `SPSR_EL0` (processor state)

## Interrupt controller (GICv2)

- QEMU `virt` with `gic-version=2`; distributor at `0x08000000`, CPU interface at `0x08010000`
- Initialization: enable distributor, enable CPU interface, set group 0 priority mask, disable all interrupts, then enable the required ones
- Interrupt acknowledgement: read `ICCIAR_EL1` (or the memory-mapped CPU interface register), spurious ID 1023 handled safely
- End-of-interrupt: write `ICCEOIR_EL1`
- Timer interrupt: IRQ 27 (physical timer), acknowledged like any other interrupt
- Unexpected interrupts: logged with the interrupt ID, then end-of-interrupt so the machine keeps running

## Exit criteria

- ARM64 exception vector installed via `VBAR_EL1`
- Synchronous exceptions decoded from `ESR_EL1`
- Data-abort diagnostics work (fault address and status printed)
- Instruction-abort diagnostics work
- Invalid user access isolated (process terminated, machine survives)
- Interrupt controller initialized
- Timer interrupts delivered (IRQ 27)
- Unexpected interrupts handled safely
- ARM64 interrupt tests pass
