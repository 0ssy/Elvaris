# Elvaris System Update Strategy

## Overview

System updates use an A/B inactive-slot model. The active system continues
running while the update is staged in an inactive slot. After verification and
a reboot, the new system becomes active. If the new system fails to boot or
pass its health check, the system rolls back to the previous known-good slot.

## Slots

| Slot | Purpose |
|------|---------|
| A    | Active system (currently running) |
| B    | Inactive system (update target) |

Each slot contains a complete, bootable Elvaris system image:
kernel, system services, filesystem, native applications, package database,
trust configuration, and configuration files.

## Update Stages

```text
1. Receive update (download or local transfer)
2. Verify update signature
3. Validate architecture and version
4. Check dependencies
5. Stage inactive system (write to inactive slot)
6. Verify staged files and metadata
7. Set next-boot marker to inactive slot
8. Reboot into the new system
9. Run startup health check
10. Confirm successful startup (mark new slot active)
```

## Next-Boot Marker

The next-boot marker is a small data structure stored in a known location
(e.g., a dedicated partition or a fixed filesystem path). It indicates which
slot to boot on the next restart. Before the health check passes, the marker
points to the new slot but the system can fall back to the previous slot.

## Health Check

The startup health check verifies:

1. Kernel started
2. Filesystem mounted
3. init started
4. Required services started
5. Package database readable
6. Security initialization succeeded

Only after the health check succeeds is the new system considered active.

## Failure Handling

| Failure | Action |
|---------|--------|
| Invalid signature | Reject update; do not stage |
| Wrong architecture | Reject update; do not stage |
| Corrupt image | Reject update; do not stage |
| Insufficient storage | Reject update; report error |
| Interrupted write | Discard staging; leave old slot intact |
| Power loss during staging | On next boot, detect incomplete stage and discard |
| Boot failure | Roll back to previous slot |
| Service startup failure | Roll back to previous slot |
| Health-check timeout | Roll back to previous slot |
| Rollback failure | Attempt recovery diagnostics; halt |

## Rollback Process

1. Detect that the new slot failed the health check.
2. Change the next-boot marker back to the previous slot.
3. Reboot into the previous known-good system.
4. Mark the failed slot as invalid.
5. Report the failure reason via diagnostics.

## Recovery

The system should be able to:

- Boot the previous known-good image
- Identify an incomplete update
- Cancel a staged update
- Restore package database consistency
- Report the failure reason
- Preserve user data where possible
- Prevent repeated boot loops (limit rollback attempts)

## Update Image Format

A system update image contains:

```text
Boot components
Kernel
System services
Filesystem image
Native applications
Package database
Trust configuration
Configuration files
```

The image is signed and architecture-specific (separate images for x86-64 and
ARM64).

## What Is Not Supported

- Delta updates
- Distributed rollback coordination
- Multiple simultaneous system versions beyond the required A/B design
- Automatic hardware-driver installation
- Secure boot hardware integration