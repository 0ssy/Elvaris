# Release Scope — Elvaris v1.0 (Milestone 22.0)

## Release Identity
- Release name: Elvaris
- Version: v1.0.0
- Release policy: Stabilization-focused, no major feature additions post-freeze

## Supported Profiles
- [x] Command-line profile
- [x] Server profile
- [x] Desktop profile
- [x] Embedded profile
- [x] Development profile
- [x] Recovery profile

All profiles must have reproducible build, bootable image, documented config, test suite, recovery procedure, known hardware target.

## Supported Architectures
- x86-64: Target Release-supported (validated via QEMU; physical as documented)
- ARM64: Target Development-supported/Validated where tested

Support levels: Builds, Boots, Development-supported, Validated, Release-supported. Only validated with evidence gets higher levels.

## Supported Hardware
See docs/hardware-support-matrix.md (to be completed in 22.8)

## Supported Applications
See docs/application-support-matrix.md (established in 21.9) - native, portable C/Rust, POSIX selected, Linux selected, Windows proof (if claimed), with evidence-based levels.

## Major Known Limitations
- Compatibility is selective, not universal
- Windows support is research/proof-based only
- Some subsystems experimental or partial
- Physical hardware support limited to documented targets

## Feature Freeze
Post-freeze: only correctness, security, data-loss, reproducibility, docs/tests/tooling fixes. Major features/ABI changes require release impact review. Changes classified by Critical security fix / Release blocker / Correctness fix / Test improvement / Documentation / Non-blocking / Deferred.
