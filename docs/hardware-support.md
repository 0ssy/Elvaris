#
 Elvaris Hardware Support

## Selected Hardware Targets

### Target 1: QEMU x86-64 (primary)

| Field | Value |
|-------|-------|
| Architecture | x86-64 |
| Machine type | QEMU `q35` |
| Firmware | OVMF (UEFI) |
| Memory | 512 MiB |
| Graphics | bootloader-provided framebuffer (VGA/QXL) |
| Storage | virtio-blk-pci |
| Network | none (disabled in test runner) |
| USB controller | not yet configured |
| Audio controller | not yet configured |
| Battery | none (desktop-class VM) |
| Sleep support | not yet tested |
| Debug | isa-debug-exit port 0x402, debugcon port 0x403 |
| Kernel target | `x86_64-unknown-none` |

QEMU PCI enumeration is the initial device-discovery source. The `virtio-blk-pci` storage device (vendor `0x1AF4`, device `0x1042` modern / `0x1001 transitional`) is already discovered by `Kernel/src/pci.rs`.

QEMU invocation is defined by `os/src/main.rs`: `qemu-system-x86_64 -machine q35 -m 512 -nographic -no-reboot -net none`, with the disk image on `virtio-blk-pci` and `isa-debug-exit` for deterministic exit codes.

### Target 2: QEMU ARM64 (validation target where practical)

| Field | Value |
|-------|-------|
| Architecture | ARM64 |
| Machine type | QEMU `virt` |
| Firmware | UEFI where available |
| Discovery source | device tree / platform firmware (to be confirmed) |

ARM64 device discovery and resource ownership follow the same device model, but discovery sources and bus enumeration differ from x86-64 PCI. ARM64 support is validated only where practical and is not the primary target.

### Target 3: One selected physical x86-64 system (not available)

This target is not available in this environment; no physical x86-64 machine is
accessible for validation. All Milestone 19 validation therefore runs on QEMU
x86-64 (q35), the primary target. Physical hardware validation is deferred
until a physical machine is available.

Before booting physical hardware, the following must be recorded:

| Field | Value |
|-------|-------|
| CPU | TBD |
| Firmware mode | UEFI / BIOS |
| Memory | TBD |
| Graphics device | TBD |
| Audio device | TBD |
| Network device | TBD |
| Storage device | TBD |
| USB controller | TBD |
| Battery presence | TBD |
| Sleep support | TBD |
| Firmware version | TBD |
| Device identifiers | TBD |

Preparation before booting physical hardware:
- Verify the image checksum.
- Prepare recovery media.
- Confirm serial or alternate diagnostic access.
- Back up any important data.
- Document firmware settings.
- Confirm the test device can be safely rebooted.
- Record the exact hardware identifiers.

## Device Identifiers in Use

Known QEMU/q35 device identifiers:

| Device | Vendor ID | Device ID | Driver |
|--------|-----------|-----------|--------|
| virtio-blk (transitional) | 0x1AF4 | 0x1001 | block/virtio |
| virtio-blk (modern) | 0x1AF4 | 0x1042 | block/virtio |

Additional identifiers are recorded as devices are discovered and bound.

## Unsupported Hardware Behavior

Devices discovered but without a driver remain in the `Unbound` state. They are listed in diagnostics but are not initialized. An unknown device class is treated as `Other` and is not claimed by default. A device whose resources conflict with an already-owned resource is not bound; the conflict is logged and the existing owner keeps the resource.

## Discovery Source Priority (initial)

1. PCI configuration space (x86-64)
2. ACPI (x86-64, for power/battery information)
3. Device tree or platform firmware (ARM64)

USB descriptors and platform-specific enumeration are added in later phases.
