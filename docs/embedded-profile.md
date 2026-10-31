# Embedded Product Profiles

Elvaris embedded support is organized into distinct product profiles instead of
one ambiguous "embedded mode." Each profile has its own service set, resource
budget, and intended deployment context.

## 3.1 Minimal profile

For the smallest resource budget:

```text
Kernel
Memory manager
Required drivers
Minimal filesystem
Init
One configured service
Watchdog
Logging
Recovery path
```

Excluded:

- Desktop
- Graphical session
- Package manager at runtime
- General shell
- Audio
- Unneeded networking
- Developer diagnostics

## 3.2 Headless service profile (primary Milestone 20 target)

For networked embedded devices:

```text
Kernel
Storage
Network
Security
Update service
Logging
Watchdog
Configured application
```

## 3.3 Interactive development profile

For bring-up and debugging; not for production deployment:

```text
Kernel
Serial console
Shell
Diagnostics
Filesystem tools
Network tools
Device inspection
Application loader
```

## 3.4 Optional graphical embedded profile

Not required for Milestone 20. If the selected board has a display, document
graphics compatibility separately and reuse Milestone 18 where practical. Do not
expand Milestone 20 into a second desktop milestone.

## Profile selection

Each embedded image is built for exactly one profile. The profile determines:

- Which services start at boot
- Which kernel features are enabled or disabled
- Default resource budgets
- Default read-only / writable layout
- Default logging level and destinations
- Default watchdog and health-check policy

No profile is a superset that silently includes desktop or graphical components.
