# Embedded Updates

## Objective

Adapt the Milestone 17 signed update system for embedded devices with
power-loss and unattended-update concerns on the primary target.

## Update model

```text
Receive update
    ↓
Verify signature
    ↓
Verify target and version
    ↓
Verify storage capacity
    ↓
Stage inactive system
    ↓
Verify staged image
    ↓
Mark pending activation
    ↓
Reboot
    ↓
Run health check
    ↓
Confirm or roll back
```

## Embedded update requirements

- Offline update (local media)
- Local update media update
- Network update if configured
- Signed images
- Target compatibility verification
- Version policy
- Downgrade policy
- Interrupted-update recovery
- Boot-attempt counter
- Automatic rollback
- Manual recovery

## Power-loss safety

The update strategy remains safe if power is lost during:

- Download → staged file is partial; discarded on next boot
- Verification → staged image not marked valid; previous image intact
- Staging → previous known-good image unchanged
- Slot metadata update → atomic write; rollback on failure
- Reboot → boot-attempt counter increments; rollback after threshold
- First boot → health check gates confirmation
- Health check → unconfirmed activation rolls back

## Exit criteria

- [ ] Embedded update layout documented
- [ ] Signed update verifies
- [ ] Wrong-target update is rejected
- [ ] Power-loss staging behavior is safe
- [ ] Failed activation rolls back
- [ ] Boot-loop prevention works
- [ ] Offline update works
- [ ] Update permissions are enforced
- [ ] Embedded update tests pass
