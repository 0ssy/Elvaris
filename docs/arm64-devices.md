# ARM64 Devices

## Console

- Device: PL011-compatible UART (`ARM,PL011` compatible string)
- Base address on QEMU `virt`: `0x09000000`
- Baud rate: 115200, 8N1
- Registers (offsets from base): DR `0x00`, FR `0x18`, LCR_H `0x2C`, CR `0x30`, IMSC `0x38`, ICR `0x44`
- Initialization: disable UART, set baud (integer + fractional baud-rate divisors), 8N1 line control, enable TX/RX
- Byte transmission: wait for TXFF clear in FR, write DR
- String output: write bytes one at a time, translating `\n` to `\r\n` via the generic console layer
- Byte reception: poll RXFE in FR; return `None` when empty (non-blocking)
- Device error handling: overrun/framing/parity status read from FR/RSR, cleared via ICR
- Debug-only verbose output: behind a compile-time debug flag

## Storage

- Initial filesystem: MemFs in RAM; no block device required for `init` and the shell
- Optional block device: QEMU virtio-mmio (`-device virtio-mmio-device,drive=...`); an ARM64-specific backend provides discovery, block reads/writes, reset, errors, shutdown behind the generic `BlockDevice` interface
- Device memory is mapped as Device-nGnRnE and is not user-accessible

## Cache and TLB maintenance

- `dc cvau` / `dc cvac` for cache clean when device memory is written by the kernel
- `tlbi vmalle1is` + `dsb ish` + `isb` after page-table or permission changes
- `isb` after writing `SPSR`/`ELR`/`VBAR`/`MAIR`/`TCR`/`TTBR` system registers

## Atomic / memory-ordering details

- Release/acquire semantics via `ldr`/`str` with appropriate barriers; `dmb ish` where multiprocessor visibility matters (SMP is out of scope, barriers kept minimal and correct for uniprocessor)
