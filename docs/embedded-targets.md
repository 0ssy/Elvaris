# Embedded Targets

## Primary target (Milestone 20)

| Field | Value |
|-------|-------|
| Target | x86-64 QEMU q35 (embedded-configured) |
| Architecture | x86-64 |
| Machine type | QEMU `q35` |
| Firmware | OVMF (UEFI) |
| Memory | 256 MiB (minimum embedded configuration) |
| Storage | VirtIO block (`virtio-blk-pci`) |
| Network | VirtIO network (`virtio-net-pci`), static or DHCP |
| Serial | ISA debug-exit port 0x402, debugcon port 0x403 |
| Watchdog | QEMU i6300esb or simulated watchdog |
| Graphics | None (headless) |
| Kernel target | `x86_64-unknown-none` |
| Support level | Boots (Milestone 20 goal: Validated) |

QEMU invocation (embedded, headless):

```text
qemu-system-x86_64 -machine q35 -m 256 -nographic -no-reboot \
  -netdev user,id=net0 -device virtio-net-pci,netdev=net0 \
  -drive file=embedded.img,format=raw,if=virtio \
  -isa-debug-exit -debugcon stdio
```

## Secondary target

None selected for Milestone 20. A physical ARM64 or x86-64 board may be added
later once the primary target is validated.

## Target identifiers

Recorded for the primary target:

| Device | Vendor ID | Device ID | Driver |
|--------|-----------|-----------|--------|
| virtio-blk (modern) | 0x1AF4 | 0x1042 | block/virtio |
| virtio-blk (transitional) | 0x1AF4 | 0x1001 | block/virtio |
| virtio-net (modern) | 0x1AF4 | 0x1041 | net/virtio |
| virtio-net (transitional) | 0x1AF4 | 0x1000 | net/virtio |

## Unsupported hardware behavior

Devices discovered but without a driver remain `Unbound`. They are listed in
diagnostics but are not initialized. An unknown device class is treated as
`Other` and is not claimed by default. A device whose resources conflict with an
already-owned resource is not bound; the conflict is logged and the existing
owner keeps the resource.

## Support levels

| Level | Meaning |
|-------|---------|
| Not supported | No code path exists; target is not in scope. |
| Builds only | The kernel compiles for the target but does not boot. |
| Boots | The kernel boots and reaches init on the target. |
| Development supported | Boot, console, storage, and basic services work; suitable for bring-up. |
| Validated | Boot, services, updates, recovery, and failure-injection tests pass reproducibly. |
| Production candidate | Validated plus security hardening, signed images, and production review. |

A target is not "supported" merely because the kernel builds for it.

## Discovery source priority (initial)

1. PCI configuration space (x86-64)
2. ACPI (x86-64, for power/battery information where present)
3. Device tree or platform firmware (ARM64; not used by the primary x86-64 target)
