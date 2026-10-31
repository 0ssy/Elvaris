# Embedded Recovery

## Objective

Ensure an embedded device remains recoverable after application, filesystem,
update, or service failure on the primary target.

## Recovery modes

| Mode | Purpose | Starts failed application? |
|------|---------|----------------------------|
| Normal mode | Regular operation | Yes |
| Safe mode | Minimal kernel, serial console, recovery storage, device inspection, rollback, log export, config reset | No |
| Recovery mode | Known-good image selection, update rollback, filesystem repair | No |
| Factory-reset mode | Explicit, authorized reset | No |

## Safe mode

Safe mode may start:

- Minimal kernel
- Serial console
- Recovery storage
- Device inspection
- Update rollback
- Log export
- Configuration reset

It must not automatically start the failed application.

## Recovery triggers

Enter safe or recovery mode after:

- Repeated boot failure (boot-loop counter threshold)
- Failed health check
- Corrupt system image
- Invalid update
- Watchdog reset threshold
- Unrecoverable configuration
- Required storage failure

## Factory reset

Factory reset does not erase user data automatically. If supported, it requires:

- Explicit authorization
- Confirmation mechanism
- Documented data behavior
- Recovery if reset is interrupted

## Exit criteria

- [ ] Recovery modes documented
- [ ] Failed update enters recovery
- [ ] Repeated boot failure enters safe mode
- [ ] Failed application can be disabled
- [ ] Logs available in recovery
- [ ] Recovery can select a known-good image
- [ ] Recovery does not erase data unexpectedly
- [ ] Safe-mode tests pass
