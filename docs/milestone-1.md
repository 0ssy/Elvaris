# Elvaris Milestone 1 Checklist

## Milestone 1: x86-64 Kernel Boot (provisional scope)

Target: Elvaris v0.1 boots in QEMU on x86-64, enters the kernel, prints diagnostic output, and shuts down safely.

- [x] x86-64 chosen as first implementation target
- [x] Existing bootloader chosen as initial boot method
- [ ] Rust bare-metal target selected (`x86_64-unknown-none`)
- [ ] Boot method selected (existing bootloader; exact one TBD)
- [ ] QEMU machine selected
- [ ] Kernel entry contract documented
- [ ] Linker strategy understood
- [ ] Serial output strategy understood
- [ ] Panic behavior defined
- [ ] Kernel builds without the host operating system
- [ ] Kernel image is produced
- [ ] Kernel boots in QEMU
- [ ] Diagnostic message appears
- [ ] QEMU exits or kernel halts safely

## First kernel boundary (allowed contents only)

- Kernel entry
- Early logging
- Architecture identification
- Panic handling
- Basic halt or shutdown behavior

## Explicitly excluded from Milestone 1

- Networking
- Filesystems
- GUI
- Package management
- Windows compatibility
- Advanced scheduling
- Physical hardware support
- Complex user-space services
- ARM64 (deferred to Milestone 2, provisional)
