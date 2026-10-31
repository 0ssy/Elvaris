# Embedded Resource Budgets

## Objective

Define bounded, predictable resource budgets for the embedded profiles on the
primary target.

## Resource budgets

| Resource | Minimal profile | Headless service profile | Development profile |
|----------|-----------------|--------------------------|---------------------|
| Minimum RAM | TBD | TBD | TBD |
| Kernel image size | TBD | TBD | TBD |
| Kernel heap | TBD | TBD | TBD |
| User memory | TBD | TBD | TBD |
| Page tables | TBD | TBD | TBD |
| Maximum processes | TBD | TBD | TBD |
| Maximum threads | TBD | TBD | TBD |
| Maximum handles | TBD | TBD | TBD |
| Maximum packet buffers | TBD | TBD | TBD |
| Maximum filesystem cache | TBD | TBD | TBD |
| Maximum log buffer | TBD | TBD | TBD |
| Maximum update staging space | TBD | TBD | TBD |
| Maximum application memory | TBD | TBD | TBD |

Values to be filled in during Phase 20.15 measurements.

## Kernel configuration

Unused features disabled per profile:

- Graphics (disabled in all embedded profiles)
- Audio (disabled in all embedded profiles)
- USB (disabled unless required by target)
- Desktop services (disabled)
- Interactive shell (disabled in minimal/headless; enabled in development)
- Unused filesystems (disabled)
- Unused network protocols (disabled)
- Debug logging (disabled in production)
- Package installation at runtime (disabled)
- Development diagnostics (disabled in production)

Every supported configuration is built and tested; no untested
conditional-compilation combinations.

## Memory behavior

Defined:

- What happens when memory is low
- Which caches may be reclaimed
- Whether process creation is rejected
- Whether services restart
- Whether the watchdog is triggered
- Whether the system enters a safe state

## Allocation rules

Avoid unbounded allocations in:

- Interrupt handlers
- Device paths
- Packet receive paths
- Logging paths
- Watchdog paths
- Recovery paths

## Exit criteria

- [ ] Resource budgets documented
- [ ] Minimal profile builds
- [ ] Headless profile builds
- [ ] Development profile builds
- [ ] Memory limits enforced
- [ ] Low-memory behavior defined
- [ ] No unbounded critical-path allocation
- [ ] Image-size report exists
- [ ] Runtime-memory report exists
- [ ] Profile resource tests pass
