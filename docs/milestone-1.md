# Elvaris Milestone 1 Checklist

## Milestone 1: x86-64 Kernel Boot

Target: Elvaris v0.1 boots in QEMU on x86-64, enters the kernel, prints
diagnostic output, and halts safely.

- [x] x86-64 chosen as first implementation target
- [x] Existing bootloader chosen as initial boot method (`bootloader` 0.11.17)
- [x] Rust bare-metal target selected (`x86_64-unknown-none`)
- [x] Boot method selected (UEFI via the `bootloader` crate)
- [x] QEMU machine selected (q35, virtio-blk-pci for the disk)
- [x] Kernel entry contract documented (`docs/bootloader.md`)
- [x] Linker strategy understood (default linker script; `entry_point!` emits `_start`)
- [x] Serial output strategy understood (direct COM1 port writes)
- [x] Panic behavior defined (write "panic" to serial, then HLT loop)
- [x] Kernel builds without the host operating system
- [x] Kernel image is produced (`uefi.img`, GPT + FAT12 + BOOTX64.EFI)
- [x] Kernel boots in QEMU
- [x] Diagnostic message appears
- [x] QEMU exits or kernel halts safely

## First kernel boundary (allowed contents only)

- Kernel entry via `bootloader_api::entry_point!`
- Early serial logging on COM1 (I/O port 0x3F8)
- Architecture identification (x86-64)
- Panic handling
- Basic halt behavior (`hlt` loop)

## Diagnostic output (verified 2026-10-18)

```
Elvaris kernel: started on x86-64
Elvaris kernel: boot information received
Elvaris kernel: usable memory regions: 94
```

## Serial console decision

Milestone 1 uses **direct serial-port output from the kernel** as the primary
diagnostic channel.

- Port: COM1, I/O base 0x3F8
- Method: direct `out dx, al` instructions; no UART initialization needed
  because the `bootloader` crate already enables serial logging before it
  hands control to the kernel.
- Why not the framebuffer: the bootloader does set up a framebuffer (1280x800
  Bgr in this run), but capturing and automating serial output in QEMU is far
  simpler. The framebuffer can be revisited later.

## QEMU launch command (verified working)

```sh
qemu-system-x86_64 \
  -machine q35 \
  -m 512 \
  -nographic \
  -no-reboot \
  -net none \
  -drive if=pflash,unit=0,format=raw,file=ovmf-x86_64/OVMF_CODE.fd,readonly=on \
  -drive if=pflash,unit=1,format=raw,file=ovmf-x86_64/OVMF_VARS.fd \
  -drive if=none,format=raw,file=<uefi.img>,id=hd0 \
  -device virtio-blk-pci,drive=hd0,bootindex=2
```

Note: `-drive if=none` exposes a bare block device with no controller, which
OVMF cannot see ("No bootable option or device was found"). Attaching the same
image to a `virtio-blk-pci` device makes it visible to the firmware.

## Explicitly excluded from Milestone 1

- Networking
- Filesystems
- GUI
- Package management
- Windows compatibility
- Advanced scheduling
- Physical hardware support
- Complex user-space services
- ARM64 (deferred to Milestone 2)