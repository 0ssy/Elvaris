# Hardware Support Matrix (Milestone 22.0/22.8)

| Target | Architecture | Firmware | Boot method | Storage | Console | Graphics | Input | Network | Audio | Power | USB | Embedded features | Support level | Known limitations | Last verified |
|--------|--------------|----------|-------------|---------|---------|----------|-------|---------|-------|-------|-----|-------------------|---------------|-------------------|---------------|
| QEMU x86_64 (q35) | x86_64 | UEFI/OVMF | UEFI/bootloader | virtio-blk | serial/vga | virtio-gpu | ps2/kbd/mouse | virtio-net | virtio-snd (if avail) | ACPI (basic) | virtio/ehci (limited) | embedded-configured supported | Development/Validated | Emulated hardware only | milestone-21 |
| QEMU ARM64 (virt) | aarch64 | UEFI/virt | UEFI/bootloader | virtio-blk | serial/pl011 | virtio-gpu | virtio-input | virtio-net | limited | PSCI/basic | limited | partial | Development-supported | Limited coverage | milestone-21 |
