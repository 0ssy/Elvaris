# Windows Compatibility

## Strategy (Research)
Porting open-source apps → selected user-space API translation → executable loading research → VM fallback. No promise for arbitrary commercial apps.

## Initial API Scope (narrow)
Process startup, basic file ops, memory alloc, console, basic window/input, basic sockets. Avoid Win32 full, DirectX, COM, registry, drivers, services, anti-cheat, DRM, .NET, Office.

## Research
Headers, arch, import tables, relocations, startup, DLLs, TLS, exception handling, manifests.
