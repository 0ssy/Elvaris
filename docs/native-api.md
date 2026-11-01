# Native API

## Versioning
- API version, ABI version defined and reported
- Deprecation policy, breaking-change policy
- Compatibility shims where needed
- Minimum supported runtime version

## Public API Documentation Template
For each public API: Purpose, Version, Arguments, Return values, Errors, Permissions, Blocking behavior, Threading behavior, Resource ownership, Compatibility promise

## Stability
Native ABI/API must be stable before broad compatibility work. Tests cover builds/packages/launch/permissions on x86-64 and ARM64.
