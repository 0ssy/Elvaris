# Embedded Storage and Filesystem

## Objective

Provide reliable storage for the selected embedded target (x86-64 QEMU q35,
VirtIO block).

## Storage requirements

The primary embedded target uses QEMU VirtIO block (`virtio-blk-pci`), reused
from Milestone 19 block/virtio support. No other storage medium is required for
Milestone 20.

## Storage policy

| Partition / region | Purpose | Mount |
|--------------------|---------|-------|
| Boot partition | Boot metadata, kernel, hardware description | Read-only, boot firmware |
| System partition | Immutable system files, binaries, libraries, config defaults | Read-only |
| State partition | Device identity, logs, application data, update metadata, recovery state | Writable |
| Update staging area | Staged inactive system image | Writable, temporary |
| Recovery partition | Known-good recovery image, safe-mode tools | Read-only |

Mount order: boot → system → state → update staging → recovery.

## Power-loss behavior

Documented assumptions for power loss during:

- State write: Writes use atomic file replacement where possible; partial state
  is isolated and treated as corrupt.
- Log write: Logs use bounded ring buffer; partial entries discarded.
- Update staging: Staged image is not activated until verified; power loss
  leaves the previous known-good image intact.
- Slot activation: Activation metadata written atomically; rollback on failure.
- Filesystem mount: Mount failure enters recovery behavior.
- Application data update: Writes to designated state paths only.

## Data integrity

- Checksums where available
- Atomic file replacement
- Bounded write retries
- Safe unmount or flush behavior on shutdown

## Exit criteria

- [ ] Selected embedded storage initializes
- [ ] Partitions or regions are documented
- [ ] System mounts read-only
- [ ] Writable state mounts safely
- [ ] Power-loss assumptions are documented
- [ ] Corrupt storage is detected
- [ ] Storage failure enters recovery behavior
- [ ] Storage tests pass
