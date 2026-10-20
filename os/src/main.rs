//! Elvaris image runner.
//!
//! The build script produces `uefi.img` (a GPT disk image containing the
//! UEFI bootloader and the compiled kernel). This binary launches QEMU with
//! the OVMF firmware, the image attached to a virtio-blk device, and the
//! debugcon device connected to stdout so the kernel's exit status code is
//! visible to the host.

use std::process::Command;

fn main() {
    let image = std::env::var_os("UEFI_PATH")
        .expect("UEFI_PATH not set by build script");
    let ovmf_code =
        std::env::var_os("OVMF_CODE_PATH").unwrap_or_else(|| "ovmf-x86_64/OVMF_CODE.fd".into());
    let ovmf_vars =
        std::env::var_os("OVMF_VARS_PATH").unwrap_or_else(|| "ovmf-x86_64/OVMF_VARS.fd".into());

    let status = Command::new("qemu-system-x86_64")
        .args([
            "-machine", "q35",
            "-m", "512",
            "-nographic",
            "-no-reboot",
            "-net", "none",
            "-drive",
            &format!("if=pflash,unit=0,format=raw,file={},readonly=on", ovmf_code.display()),
            "-drive",
            &format!("if=pflash,unit=1,format=raw,file={}", ovmf_vars.display()),
            "-drive",
            &format!("if=none,format=raw,file={},id=hd0", image.display()),
            "-device", "virtio-blk-pci,drive=hd0,bootindex=2",
            // The kernel writes a single exit status byte to port 0x402. The
            // isa-debug-exit device makes QEMU terminate with that byte as its
            // exit code, so automated tests can distinguish success, panic,
            // unexpected halt, and timeout. Exactly one byte is written: the
            // device terminates on the first write, and further writes race
            // with the shutdown and make the reported code nondeterministic.
            "-device", "isa-debug-exit,iobase=0x402,iosize=1",
            // Keep the debugcon on a separate port so that it does not
            // collide with the debug-exit device.
            "-debugcon", "stdio",
            "-global", "isa-debugcon.iobase=0x403",
        ])
        .status()
        .expect("failed to launch QEMU");

    // QEMU exits with `(exit_byte << 1) | 1` when the isa-debug-exit device
    // receives a byte. The kernel writes `EXIT_SUCCESS` (0x10) on a clean
    // boot, so the expected host code is `(0x10 << 1) | 1` = 33. A panic
    // writes 0x11 and yields 35; anything else is a failure.
    let code = status.code().expect("QEMU was killed by a signal");
    assert_eq!(code, 33, "QEMU exited with unexpected code {}", code);
}