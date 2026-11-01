# Linux Compatibility

## Execution Model
Recompilation first → selected syscall translation → VM/container fallback later. Do not run arbitrary Linux binaries initially.

## Binary Compatibility (if used)
Define format, arch, dynamic linking, interpreter, relocations, TLS, mmap, startup, exit.

## Syscall Policy
Only implement calls needed by selected apps. Track Implemented/Partially/Translated/Rejected/Unsupported. Unsupported must fail predictably.

## Target Apps
CLI util, text editor, network client/server, build tool, small GUI (if graphics ready). Avoid kernel modules, privileged devices, complex namespaces etc.
