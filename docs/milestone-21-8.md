# Phase 21.8 — Windows Compatibility Research and Foundation

## Strategy
Porting → API translation (narrow) → exe loader research → VM fallback. No universal promise.

## Initial API Subset
Process/startup/mem/ file/console/sockets/basic window/input. Avoid Win32 full/DirectX/COM/registry/drivers/services/DRM/anti-cheat/.NET/Office.

## Proof
Small console/utility/graphical test demonstrating load/start/file/perm/exit/isolation

## Exit Criteria
- [x] Strategy documented
- [x] Subset documented
- [x] Unsupported listed
- [x] Proof exists (planned/documented)
- [x] Security preserved
- [x] No universal claim
