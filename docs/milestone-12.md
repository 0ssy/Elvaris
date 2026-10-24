# Milestone 12 — Device Input and Terminal Subsystem

Status: complete.

## Architecture

```text
Serial hardware (COM1)
    ↓
serial::try_read_byte / serial::write_bytes
    ↓
Console InputSource / Output traits (console.rs)
    ↓
read_line_from terminal line discipline
    ↓
Shell input (syscall read on fd 0)
    ↓
Command execution
    ↓
Console output (syscall write on fd 1/2)
```

## Components

- `Kernel/src/serial.rs`: COM1 driver, input via `try_read_byte`, output via `write_bytes`.
- `Kernel/src/console.rs`: `InputSource`, `Output` traits; `SerialInput`, `SerialOutput`; `read_line_from` line discipline (backspace, CR/LF normalization, max length).
- `Kernel/src/user/programs/shell.rs`: shell reads lines via console syscall.

## Line discipline rules

- Max line length: 1024 bytes (`MAX_LINE_LENGTH`).
- Enter (CR or LF) submits line; CRLF normalized to single submit.
- Backspace (0x08/0x7F) removes last char.
- Printable bytes appended and echoed.

## Control keys

- Ctrl-C / Ctrl-D not yet fully implemented; deferred. Unsupported controls ignored safely.

## Error handling

- Empty input returns `None` (exhausted). Overflow truncates. No panics on malformed input.

## Tests

- Unit tests in console.rs cover line ending, CR ignore, backspace, truncation, exhausted source, write_to LF translation.
- Userland tests cover shell built-ins, external programs, exit status.

## Limitations

- No keyboard/USB input.
- No command history, tab completion, cursor movement.
- No multiple terminals.
- Ctrl-C/Ctrl-D full semantics deferred.
- Polling-based input only.
