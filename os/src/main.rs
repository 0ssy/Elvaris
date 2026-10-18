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
            // The kernel writes a 4-byte exit status to port 0x402. Connect
            // the debugcon to stdout so the host can read it.
            "-debugcon", "stdio",
            "-global", "isa-debugcon.iobase=0x402",
        ])
        .status()
        .expect("failed to launch QEMU");

    assert!(status.success(), "QEMU exited with status {:?}", status.code());
}