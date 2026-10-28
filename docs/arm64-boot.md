# ARM64 Boot Path

## Target machine

QEMU `virt` machine (`-machine virt,gic-version=2,highmem=off`). The `virt` machine is a predictable virtual ARM64 platform with standard virtual devices (PL011 UART, GICv2, generic timer, virtio-mmio block).

## Firmware

No firmware is required. QEMU loads the kernel ELF directly with `-kernel` and transfers control at EL2 (AArch64 hypervisor mode). The kernel entry address is the ELF entry symbol (`_start`).

## Initial state on entry

- Exception level: EL2 (64-bit)
- Stack: none valid; the boot entry installs a temporary stack before any call
- MMU: disabled
- Caches: not guaranteed; the boot entry invalidates/inits as needed before enabling the MMU
- Interrupts: masked (all exception routes disabled until the generic kernel installs them)
- Available CPU features: read from ID registers (`ID_AA64PFR0_EL1`, `ID_AA64MMFR0_EL1`, etc.) and reported in diagnostics
- Device tree: `x0` holds the physical address of the flattened device tree blob (FDT) provided by QEMU
- UEFI memory map: not applicable (no UEFI); memory regions come from the device tree `/memory` node and `/reserved-memory`
- Kernel entry calling convention: `x0` = FDT physical address; control arrives at `_start` in EL2
- Halt: `wfi`/`wfe` loop after `shutdown: ready`; QEMU is terminated by the test harness

## Boot-information structure delivered to the generic kernel

The ARM64 boot path fills an architecture-neutral `BootInfo`:

- Memory regions: usable, reserved, bootloader/firmware, device (parsed from the device tree)
- Kernel location: ELF load address
- Kernel size: ELF file size
- Initial stack information: top and size of the early stack
- Console information: PL011 base address and baud rate
- Hardware-description information: device tree present, compatible string ("linux,dummy-virt"), machine model
- Boot arguments: none (deferred)
- Firmware or bootloader identifier: "qemu-virt-direct-kernel"

The structure follows the existing boot contract (`docs/boot-contract.md`) so the generic kernel can consume it unchanged.

## Boot questions answered

- Initial exception level: EL2
- Initial stack: temporary 4 KiB stack in the BSS/boot area, replaced by the permanent kernel stack before `kernel_main`
- MMU state: off on entry; on before the generic kernel runs
- Cache state: architecturally unknown; invalidated before MMU enable
- Interrupt-mask state: masked on entry; unmasked only after the exception vector is installed
- Available CPU features: reported from ID registers
- Device-tree location: `x0` on entry
- UEFI memory-map handling: not used
- Kernel entry calling convention: `x0` = FDT address, EL2, AArch64
- How the kernel returns or halts: `wfe` loop; never returns to QEMU

## Exit criteria

- ARM64 firmware/QEMU starts (`qemu-system-aarch64 -machine virt -kernel kernel.elf`)
- ARM64 boot path loads the kernel
- Kernel entry is reached (`_start` runs)
- Architecture is reported as AArch64
- Boot information is received and parsed
- Early serial output works on PL011
- Kernel halts safely in the `wfe` loop
- ARM64 boot test exits deterministically (QMP `quit` on `shutdown: ready`)
