# Compatibility Policy

## Purpose
Define exactly what compatibility means for Elvaris. This policy is binding for Milestone 21.

## Support Levels
- Unsupported: No maintained compatibility path
- Builds: Compiles for Elvaris but runtime incomplete
- Starts: Launches but significant features missing
- Basic functionality: Core operations work, important features missing
- Usable: Suitable for ordinary use with documented limitations
- Well supported: Regular tests, stable behavior, maintained packaging
- Native integration: Native Elvaris build/integration path

## Compatibility Boundaries
Must document: executable formats, foreign syscalls, filesystem emulation, graphics/audio/network APIs, permissions, foreign installers, dynamic linking, plugins, background services.

## Security Requirements
Compatibility apps subject to same isolation, permissions, resource limits, package verification, filesystem policy, recovery/updates. Must not bypass verification or gain elevated privileges.

## No Universal Claims
We do not claim "every Linux/Windows/macOS app works". Only selected apps with documented limitations.

## Principles
Native primary; compatibility is a service layer above kernel; measurable support; fail predictably on unsupported calls.
