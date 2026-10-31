# Milestone 19: Audio, Power, and Hardware Expansion

## Objective

Make Elvaris more practical on selected desktop and laptop hardware by adding hardware discovery, device ownership, USB foundations, physical keyboard and mouse support, audio output, power and battery information, sleep/resume foundations, device permissions, and hardware-specific configuration. Hardware support expands gradually; Milestone 19 does not attempt to support every device or every computer model.

## Supported Hardware Targets

| Target | Architecture | Machine | Status |
|--------|--------------|---------|--------|
| QEMU x86-64 virtual hardware | x86-64 | q35 | Primary target |
| QEMU ARM64 virtual hardware | ARM64 | virt | Validation target where practical |
| One selected physical x86-64 system | x86-64 | TBD | Selected physical target (see docs/hardware-support.md) |

QEMU virtual hardware is the initial target. Physical hardware validation happens only after QEMU paths are stable.

## Device Model

Devices are represented by a device record containing device identifier, device class, parent device, bus, vendor and product identifiers, resource requirements, driver state, power state, security policy, and ownership. Device identity does not rely only on human-readable names. See `docs/device-model.md`.

The device lifecycle is:

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

The system rejects two drivers claiming the same device, direct process access to unclaimed devices, driver use of a removed device, and stale device handles referring to a new device. Resource ownership covers MMIO regions, I/O ports, interrupt lines, DMA regions, PCI configuration access, and device power state.

## Resource Discovery

The initial discovery focus is PCI configuration space and the virtual devices required by QEMU. ACPI, device tree, firmware tables, USB descriptors, and platform-specific enumeration may be added later.

## Input Devices

Physical keyboard and mouse support preserve the existing abstract input-event model. Keyboard hardware feeds device drivers, which produce input events consumed by the input service and routed to terminal or graphical sessions. Terminal and graphical systems do not read hardware registers directly.

## USB Strategy

One USB host-controller type is implemented first. Device reset, enumeration, descriptor retrieval, configuration selection, and one basic device class (USB HID keyboard/mouse) are the initial scope. USB descriptors are treated as untrusted external input and validated before use.

## Audio Strategy

An audio abstraction is created before physical sound hardware: application → audio API/service → audio stream → mixer → audio device driver → hardware. Playback only, signed PCM samples, one or two channels, one documented sample rate. Mixing happens in a user-space audio service; applications do not access device registers or DMA buffers directly. A QEMU-compatible virtual audio device or deterministic test backend is used first.

## Power Strategy

Power state and battery information are exposed without changing power state yet. AC/external power state, battery presence, percentage, charging state, voltage/energy, and estimated time remaining where available. ACPI on x86-64 and device tree/platform firmware on ARM64 are the discovery sources. A user-space power service publishes read-only information; applications do not access ACPI or firmware tables directly.

## Hotplug Strategy

Device added, removed, reset, suspended, resumed, and driver-failure events are supported where applicable. Safe removal stops new operations, notifies clients, drains transfers, releases DMA/interrupts/mappings, then removes the device record. Stale handles are invalidated.

## Security Model

Hardware is an authority boundary. Applications cannot access hardware registers directly. Device mappings are not user-accessible without authorization. DMA buffers are isolated or validated. USB descriptors are treated as untrusted. Device removal invalidates handles. Audio clients cannot read other clients' streams. Power controls require authorization. Suspend and shutdown require authorization. Raw input access is restricted. Driver failures cannot overwrite unrelated memory. Hotplug events cannot grant privilege. Device permissions are enforced consistently on x86-64 and ARM64 where supported.

## Testing Strategy

Tests cover device discovery, identity, driver binding, duplicate-claim rejection, resource ownership, device release, failed-driver handling, stale-handle rejection, and device-state transitions. QEMU tests verify discovery, keyboard, mouse, USB, audio, power information, suspend/resume if supported, device removal, driver failure, permission denial, and clean shutdown. Physical tests repeat the supported-device matrix on the selected machine. Failure-injection tests simulate device initialization failure, timeout, queue exhaustion, DMA failure, malformed descriptor, audio underrun, battery-data corruption, device removal during use, suspend preparation failure, and resume failure.

## Recovery Behavior

If a driver fails: mark device failed, release resources where safe, log the failure, prevent unsafe retries, keep unrelated devices working. If suspend preparation fails: abort suspend, restore normal operation, report the failure. The system never enters a partial suspend state.

## Out of Scope

- All physical hardware support
- Wi-Fi driver expansion
- Bluetooth
- Cameras
- GPU acceleration
- Advanced fan control
- Full thermal management
- Hibernation to disk
- Fast boot
- USB 3 support for every controller
- Every audio codec
- Every laptop battery interface
- Multi-seat input
- Mobile-device power management
- Real-time audio guarantees
- Universal device-driver compatibility
- Hardware virtualization
- ARM64 laptop support
- Automatic driver downloads
- Firmware updating
- Thunderbolt security
- Advanced suspend-to-disk recovery

## Exit Criteria

- [ ] Hardware targets are documented
- [ ] Device model is documented
- [ ] Device discovery works
- [ ] Device ownership is enforced
- [ ] Resource conflicts are detected
- [ ] Selected physical-style input works
- [ ] USB foundation or selected USB class works
- [ ] Audio abstraction works
- [ ] Virtual audio works
- [ ] Selected audio device works
- [ ] Power information is available
- [ ] Battery absence is handled
- [ ] Suspend policy is documented
- [ ] Suspend/resume works or safely aborts
- [ ] Hotplug behavior is defined
- [ ] Device permissions are enforced
- [ ] One selected physical machine is validated
- [ ] Failure-injection tests pass
- [ ] Existing graphics tests pass
- [ ] Existing networking tests pass
- [ ] Existing security tests pass
- [ ] Existing packaging and update tests pass
- [ ] Recovery behavior is documented
- [ ] Limitations are documented

---

## Phase 19.0 — Hardware Target and Device Model Exit Criteria

- [x] First physical target is selected
- [x] QEMU device targets are documented
- [x] Device classes are defined
- [x] Device identity rules are documented
- [x] Device ownership rules are documented
- [x] Unsupported hardware behavior is defined
