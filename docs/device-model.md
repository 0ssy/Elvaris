# Elvaris Device Model

## Overview

Elvaris represents every piece of hardware as a device record. The device model is the foundation for device discovery, driver binding, resource ownership, and device permissions. Device identity does not rely only on human-readable names.

The device lifecycle:

```text
Firmware and hardware description
        ↓
Device discovery
        ↓
Device manager
        ↓
Device drivers
        ↓
User-space device services
        ↓
Applications
```

## Device Identity

A device record contains:

| Field | Type | Meaning |
|-------|------|---------|
| Device identifier | u64 | Stable identifier for the device record |
| Device class | enum | Class of the device (input, storage, network, audio, display, power, bus, bridge, other) |
| Parent device | Option<u64> | The device that owns this device on the bus/tree |
| Bus | enum | Bus the device is attached to (PCI, USB, ACPI, platform, virtual) |
| Vendor identifier | u16 | Vendor ID from the bus enumeration |
| Product identifier | u16 | Product/device ID from the bus enumeration |
| Resource requirements | Vec<Resource> | MMIO regions, I/O ports, interrupts, DMA regions |
| Driver state | DeviceState | Current lifecycle state of the device |
| Power state | PowerState | Current power state of the device |
| Security policy | DevicePolicy | Access policy for the device |
| Ownership | Option<Owner> | Current owner (driver or service) of the device |

A device identifier is assigned by the device manager at discovery time and is stable for the lifetime of the device record. Bus addresses (PCI bus/device/function, USB address, etc.) are recorded separately and may change if the device is re-enumerated.

## Device Classes

```text
Input        - Keyboard, mouse, pointer, touchpad, other input
Storage      - Block device, NVMe, SATA, virtio-blk
Network      - Ethernet, Wi-Fi (deferred), virtio-net
Audio        - Audio output device, audio input device (deferred)
Display      - Framebuffer, GPU, display controller
Power        - Battery, AC adapter, power controller
Bus          - PCI host controller, USB host controller, platform bus
Bridge       - PCI bridge, USB hub
Other        - Unclassified or unknown devices
```

## Device States

Valid device states:

```text
Discovered   - Device found by enumeration, not yet claimed
Unbound      - Device has no driver assigned
Binding      - Driver is attaching to the device
Active       - Device is initialized and usable
Suspended    - Device is suspended; operations paused
Removing     - Device is being torn down
Removed      - Device record is gone; handles are invalid
Failed       - Device or driver failed; resources released where safe
```

Valid state transitions:

| From | To | Condition |
|------|-----|-----------|
| Discovered | Unbound | Enumeration complete, resources allocated |
| Unbound | Binding | Driver claims and begins initialization |
| Binding | Active | Driver initialization succeeded |
| Binding | Failed | Driver initialization failed |
| Active | Suspended | Suspend requested and prepared |
| Suspended | Active | Resume succeeded |
| Suspended | Failed | Resume failed |
| Active | Removing | Device removal requested |
| Removing | Removed | Teardown complete |
| Any | Failed | Driver or device failure |
| Failed | Unbound | Driver released and device re-enumerated (manual recovery) |

A device may not transition directly from `Failed` to `Active`; a failed device must be released and re-bound through `Unbound`.

## Device Ownership

A device has one clear owner at a time:

```text
Unclaimed device
    ↓
Driver claims device
    ↓
Driver initializes device
    ↓
Device service uses driver
    ↓
Driver releases device
```

Ownership rules:

- Exactly one driver may claim a device at a time. Duplicate claims are rejected.
- A process may not access an unclaimed device directly.
- A driver may not use a device after it has been removed.
- A stale device handle referring to a new device is rejected. Handles are bound to a device record and a generation counter; when the device record is replaced, old handles fail validation.

## Resource Ownership

The device manager tracks these resources per device:

- MMIO regions (physical address ranges mapped into kernel space)
- I/O ports (port ranges)
- Interrupt lines (IRQs, MSI/MSI-X vectors where applicable)
- DMA regions (memory regions the device may read or write)
- PCI configuration access
- Device power state

Resource conflicts are detected at claim time. If two devices claim overlapping MMIO, I/O ports, interrupts, or DMA regions, the second claim is rejected and the conflict is logged.

## Device Record Structure

The canonical device record (conceptual):

```text
struct Device {
    id: DeviceId,
    class: DeviceClass,
    parent: Option<DeviceId>,
    bus: BusType,
    vendor_id: u16,
    product_id: u16,
    resources: ResourceSet,
    state: DeviceState,
    power_state: PowerState,
    policy: DevicePolicy,
    owner: Option<Owner>,
    generation: u64,
}
```

`generation` increments each time the device record is recreated, so stale handles are detected.

## Power States

```text
On        - Device is fully powered and operational
LowPower  - Device is in a reduced-power state (device-specific)
Off       - Device is powered off
Unknown   - Power state could not be determined
```

## Security Policy

A device policy defines which operations are allowed and by whom:

- Device owner
- Device class
- Access policy (read, write, control, raw access)
- Privileged operations
- Application permissions

Ordinary applications must not automatically access raw USB devices, audio hardware, power controls, input-device internals, PCI configuration, or DMA buffers.

## Failure Behavior

If a driver fails:

1. Mark the device `Failed`.
2. Release resources where safe.
3. Log the failure.
4. Prevent unsafe retries.
5. Keep unrelated devices working.

A failed device must not crash the kernel or leave unrelated devices in an inconsistent state.

## Unsupported Hardware

Devices that are discovered but have no driver remain in `Unbound` state. They are listed in diagnostics but are not initialized. An unknown device class is treated as `Other` and is not claimed by default.

## Diagnostics Events

Structured device events:

```
device: discovered
device: bound
device: initialized
device: failed
device: suspended
device: resumed
device: removed
```
