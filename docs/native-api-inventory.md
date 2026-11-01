# Native API Inventory (Milestone 21.1.1)

## Process & Lifecycle
- exit (SYS_EXIT) - Stable
- exec (SYS_EXEC) - Stable  
- wait (SYS_WAIT) - Stable
- get_pid/get_ppid/getuid/getgid - Stable

## Filesystem
- open/read/write/close/stat/getdents - Stable

## Console
- write to stdout/stderr - Stable

## System Info
- meminfo/procs - Stable
- shutdown/reboot - Stable

## Classification
Stable: core syscalls. Experimental/Internal as applicable. Only Stable APIs used as compatibility foundation.
