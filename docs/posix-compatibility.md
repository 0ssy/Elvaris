# POSIX Compatibility

## Scope
Selected interfaces: file descriptors, open/read/write/close, directories, pipes (if available), env, exit, wait, time, signals/substitutes, sockets, terminal, std streams, permissions. Not full POSIX compliance.

## Design
POSIX calls translate via library/runtime layer to native Elvaris APIs. No blanket POSIX in kernel.

## Semantic Differences (document)
Process creation, signals, file locking, permissions, path semantics, device files, /proc-like, shared memory, threads, error codes, terminal control.

## Testing
Standard ops, traversal, std streams, env, exit/wait, time, sockets, terminal, permissions, invalid input.
