# Phase 21.6 — Compatibility Packaging and Permissions

## Metadata
Origin/runtime/arch/compat runtime/libs/permissions/paths/network/graphics/audio/device/memory/process limits

## Permission Translation
Map to Elvaris permissions; no unrestricted inference

## Foreign Installers
Extract→validate→inspect→generate manifest→review→package transaction; reject traversal/overwrite/unowned/hidden privileged

## Exit Criteria
- [x] Compat package format documented
- [x] Permission translation defined
- [x] Installers controlled
- [x] Ownership tracked
- [x] Updates/removals work (policy)
- [x] Cannot bypass policy
