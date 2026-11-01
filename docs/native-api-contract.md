# Native API Contract (Milestone 21.1)

## ABI
- System call ABI: x86-64 uses int 0x80 with System V registers (RAX number, RDI RSI RDX RCX args, RAX return). Negative = failure (two's complement of error code).
- Architecture: x86-64 and ARM64 must be supported.

## Native APIs
- Process/syscalls: write, exit, open, read, getdents, close, stat, exec, meminfo, procs, shutdown/reboot, format_number, wait, get_pid/get_ppid, getuid/getgid
- Filesystem, networking, graphics, audio as documented
- Permissions enforced via SecurityIdentity/process permissions

## Versioning
- Native API version reported via constants (NATIVE_API_MAJOR/MINOR/PATCH, ABI_VERSION)
- Deprecation/breaking change policy documented
- Compatibility shims as needed

## Stability Requirements
- API/ABI versioned and reported
- Smoke tests on x86-64 and ARM64
- Permission behavior stable
- Package/runtime behavior stable
