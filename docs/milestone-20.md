# Milestone 20: Embedded Profile

## Objective

Create a documented, reproducible, resource-conscious embedded version of Elvaris
for selected hardware targets. The embedded profile is a separate product profile
built on the existing kernel — not a desktop installation with features removed.

The embedded profile must be able to:

```text
Build a small image
    ↓
Boot on a documented embedded target
    ↓
Discover or receive board hardware information
    ↓
Initialize only required services
    ↓
Start one configured application or service
    ↓
Operate with bounded memory and storage
    ↓
Recover from selected failures
    ↓
Receive verified updates
    ↓
Report health and watchdog status
```

## Supported Targets

| Target | Architecture | Support level | Status |
|--------|--------------|---------------|--------|
| x86-64 QEMU q35 (embedded-configured) | x86-64 | Builds, Boots | Primary target (Phase 20.0 confirmed) |
| ARM64 QEMU virt | ARM64 | Not supported | Requires ARM64 kernel port (not present); out of scope for M20 |

**Phase 20.0 decision (confirmed):**

- Primary embedded target: **x86-64 QEMU q35**, embedded-configured (small RAM, serial console, VirtIO block/network).
- Primary product profile: **headless service profile**.
- Minimum resources: to be defined in docs/embedded-targets.md.
- Boot method: QEMU firmware/bootloader, serial console, device-tree or ACPI discovery.
- Recovery method: safe mode + rollback to previous known-good image.
- Update method: signed embedded update (Milestone 17 model adapted).

Support levels: Not supported / Builds only / Boots / Development supported / Validated / Production candidate. x86-64 QEMU q35 embedded is "Boots" at minimum for M20.

## Product Profiles

- Minimal profile — kernel, memory manager, required drivers, minimal filesystem,
  init, one configured service, watchdog, logging, recovery path.
- Headless service profile — kernel, storage, network, security, update service,
  logging, watchdog, configured application (likely primary M20 target).
- Interactive development profile — serial console, shell, diagnostics, filesystem
  and network tools, device inspection, application loader (development only).
- Optional graphical embedded profile — not required for M20; documented separately
  if the selected board has a display.

## Boot Model

Documented embedded boot image separate from desktop/server images. Firmware,
entry point, memory map, device-tree location, kernel location, boot arguments,
and console configuration defined per target.

## Device-Tree Model

Controlled hardware-description interface with node traversal, compatible-string
matching, register/interrupt/clock/memory/GPIO descriptions, status fields, and
validation. Raw unchecked values are not exposed to drivers.

## Resource Budgets

Documented minimum RAM, kernel image size, kernel heap, user memory, page tables,
and maximum processes/threads/handles/buffers per profile.

## Kernel Configuration

Features may be disabled (graphics, audio, USB, desktop services, shell, unused
filesystems/protocols, debug logging, package installation). Every supported
configuration is built and tested; no untested conditional-compilation
combinations.

## Service Configuration

Explicit service manifest with startup dependencies, restart policy, health
checks, resource limits, and shutdown behavior. Services start in dependency
order, not incidental order.

## Storage Model

Selected embedded storage path (e.g. QEMU VirtIO block). Boot/system/state/update
partitions documented. Read-only system, writable state separated.

## Read-Only System Model

Immutable system mounted read-only; unauthorized writes rejected; writable state
stored separately; application data separate from system files.

## Watchdog

Hardware or simulated watchdog with health-monitor service, bounded restart
policy, health checks, and defined failure behavior (stop feeding → reset →
recovery).

## Networking

Reuses Milestone 15 networking. Static or DHCP per profile. No listening services
by default; only configured service ports enabled.

## Updates

Signed embedded updates following the Milestone 17 model adapted for power-loss
safety: receive → verify signature/target/capacity → stage inactive → verify →
mark pending → reboot → health check → confirm or roll back.

## Recovery

Normal / safe / recovery / factory-reset modes. Failed updates and repeated boot
failures enter recovery; factory reset requires explicit authorization.

## Security

Signed system images, signed packages, protected trust roots, least-privilege
services, read-only system files, device permissions, restricted debug access,
secure update authorization, bounded logs, no default unrestricted network
service. Production and development debug policies differ.

## Determinism

Startup and memory measurements recorded; reproducible image builds.

## Testing

Unit tests, QEMU embedded tests, failure-injection tests, and physical-target
tests where available.

## Out of Scope

General support for every embedded board; every ARM64 SoC; industrial/medical/
automotive certification; hard real-time guarantees by default; secure boot on
every platform; full TPM; hibernation; automatic firmware updates; Wi-Fi/
Bluetooth expansion; cameras; cloud fleet management; remote enrollment;
containers/orchestration; zero-downtime updates; automatic factory reset;
universal OTA; replacing Milestone 17 packaging or Milestone 19 device model.

## Verification Status

### Verified (v0.20-embedded-foundation)

- [x] Primary embedded target documented (x86-64 QEMU q35, headless service profile)
- [x] Embedded support levels defined
- [x] Device-tree or platform discovery works (parser unit tests pass)
- [x] Minimal profile builds (default feature set: 237 passed, 0 failed)
- [x] Headless profile builds (all features: 337 passed, 0 failed)
- [x] Development profile builds (all features: 337 passed, 0 failed)
- [x] Existing kernel tests pass with no regressions
- [x] Bare-metal x86-64-unknown-none build compiles cleanly
- [x] Resource-constrained feature gating (graphics, audio, usb, desktop, shell, packages, debug)
- [x] Related filesystem regressions fixed
- [x] Embedded image boots on QEMU q35 (boot integration test: exit code 33, all serial markers present)
- [x] Minimal profile boots (kernel initializes all subsystems without panic during QEMU boot)

### Pending — runtime completion and verification

- [ ] Minimal profile boots and starts configured service (no init/service manager yet)
- [ ] Headless profile boots and starts configured service (no init/service manager yet)
- [ ] Development profile boots (requires shell)
- [ ] Device discovery through the embedded boot path (PCI/USB/audio discovery works during boot; DT parser not wired to ACPI-based QEMU q35 boot)
- [ ] Minimal init and service supervision (not implemented)
- [ ] Service dependencies and failure isolation (not implemented)
- [ ] Read-only system and writable state separation (not implemented)
- [ ] Embedded storage behavior (storage::initialize works during boot; read-only enforcement not implemented)
- [ ] Watchdog and health monitoring (not implemented)
- [ ] Embedded networking profile (net::service::init_network() works during boot; profile-specific config not implemented)
- [ ] Embedded application contract and permissions (not implemented)
- [ ] Signed embedded updates (not implemented)
- [ ] Rollback and recovery mode (not implemented)
- [ ] Boot-loop prevention (not implemented)
- [ ] Failure-injection tests (not implemented)
- [ ] Resource and startup measurements (not implemented)
- [ ] Physical-target validation or explicit documented limitation

### Integration test results

| Test | Result |
|------|--------|
| boot | Passed (QEMU q35, exit code 33, all markers) |
| heap | 1 passed, 1 failed (pre-existing QEMU huge-page issue: `MappingFailed(HugePageUnsupported)`) |
| user | 2 passed, 0 failed |
| lib (default) | 237 passed, 0 failed |
| lib (all features) | 337 passed, 0 failed |

These items require new subsystems (init/service manager, watchdog, updates, recovery, read-only enforcement) that are not yet implemented.

## Phase Index

| Phase | Title | Status |
|-------|-------|--------|
| 20.0 | Embedded Requirements and Target Selection | Complete |
| 20.1 | Embedded Boot and Image Layout | Complete (verified via QEMU boot test) |
| 20.2 | Device-Tree and Hardware Description | Complete (verified) |
| 20.3 | Embedded Device Configuration | Pending |
| 20.4 | Resource-Constrained Kernel Profile | Complete (verified) |
| 20.5 | Minimal Init and Service Manager | Pending |
| 20.6 | Read-Only and Immutable System Model | Pending |
| 20.7 | Embedded Storage and Filesystem | Pending |
| 20.8 | Watchdog and Health Monitoring | Pending |
| 20.9 | Embedded Networking | Pending |
| 20.10 | Embedded Application Contract | Pending |
| 20.11 | Embedded Updates | Pending |
| 20.12 | Recovery and Safe Mode | Pending |
| 20.13 | Embedded Security Hardening | Pending |
| 20.14 | Optional Real-Time Scheduling | Pending |
| 20.15 | Startup and Resource Measurements | Pending |
| 20.16 | Testing and Validation | Pending |
