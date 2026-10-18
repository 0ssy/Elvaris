# Elvaris Bootloader Strategy

## Initial Bootloader Decision

Elvaris will use a pinned version of the Rust `bootloader` crate for the
first x86-64 milestone.

The bootloader will run through UEFI firmware in QEMU and load a Rust
kernel targeting `x86_64-unknown-none`.

The bootloader is a development dependency and is not considered a
permanent Elvaris architectural commitment.

ARM64 boot support is deferred until Milestone 2. The future ARM64 boot
path may use a different bootloader or platform interface, but it must
provide equivalent information through the Elvaris boot contract.

## Pinned Bootloader Details

Bootloader crate: `bootloader`
Bootloader version: `0.11.17` (pinned; published 2026-07-27)
Required Rust version: nightly `1.101.0-nightly (8006091eb 2026-10-17)`
  - The `bootloader` crate requires the unstable `artifact-dependencies`
    feature, which is only available on the nightly toolchain.
  - A `rust-toolchain.toml` in the project will pin the toolchain to this
    exact nightly so the build is reproducible.
Supported target: `x86_64-unknown-none` (kernel), `x86_64-unknown-uefi`
  (bootloader binary, built via `-Zbuild-std=core`)
Boot protocol: UEFI (the `uefi` feature of the `bootloader` crate, which
  pulls in the `gpt` crate for GPT disk images)
Firmware mode: UEFI only for this milestone. The `bootloader` crate also
  supports BIOS via its `bios` feature, but Elvaris targets UEFI.
Kernel entry requirements:
  - Kernel must be `#![no_std]` and `#![no_main]`.
  - Kernel must depend on `bootloader_api = "0.11.17"`.
  - Kernel entry point is registered with
    `bootloader_api::entry_point!(kernel_main)`.
  - The macro generates an `#[unsafe(export_name = "_start")]` function
    with signature `extern "C" fn(boot_info: &'static mut bootloader_api::BootInfo) -> !`.
  - The function name passed to the macro is arbitrary; the linker uses the
    generated `_start` symbol.
  - The configuration is serialized into a `.bootloader-config` section of
    the kernel ELF; the bootloader reads it at load time.
Boot information provided:
  - `api_version`: `ApiVersion` (must match the bootloader version).
  - `memory_regions`: map of physical memory regions (`MemoryRegions`),
    with `MemoryRegionKind::Usable` regions safe for the kernel.
  - `framebuffer`: optional `FrameBuffer` for graphical output.
  - `physical_memory_offset`: optional offset to map physical memory
    (only if `map-physical-memory` config is enabled).
  - `recursive_index`: optional level-4 page table index for recursive
    mapping (only if `map-page-table-recursively` config is enabled).
  - `rsdp_addr`: optional RSDP address for ACPI.
  - `tls_template`: optional TLS template.
  - `ramdisk_addr` / `ramdisk_len`: optional ramdisk.
  - `kernel_addr` / `kernel_len`: physical address and size of the kernel ELF.
  - `kernel_image_offset`: virtual address of the loaded kernel image.
  - `kernel_stack_bottom` / `kernel_stack_len`: kernel stack.
Image-building method:
  - The kernel is compiled with `cargo build --target x86_64-unknown-none`.
  - A top-level `os` crate uses an artifact dependency on the kernel:
    `kernel = { path = "kernel", artifact = "bin", target = "x86_64-unknown-none" }`.
  - The unstable `bindeps` feature is enabled in `.cargo/config.toml`.
  - A `build.rs` in the `os` crate calls `bootloader::UefiBoot::new(kernel_path)`
    to create a bootable GPT/FAT disk image.
  - The image contains `efi/boot/bootx64.efi`, `kernel-x86_64`, and
    optionally `boot.json` and `ramdisk`.
QEMU launch requirements:
  - `qemu-system-x86_64 -machine q35 -m 512 -nographic`
  - `-drive if=pflash,unit=0,format=raw,file=<OVMF_CODE>,readonly=on`
  - `-drive if=pflash,unit=1,format=raw,file=<OVMF_VARS>`
  - `-drive if=none,format=raw,file=<uefi-image>,id=hd0`
  - `-net none`
  - The UEFI image is a raw GPT disk; OVMF boots `efi/boot/bootx64.efi`.
Known limitations:
  - Requires nightly Rust (unstable `artifact-dependencies`).
  - The UEFI bootloader loads the kernel from a FAT filesystem on a GPT
    disk image; it does not support direct `-kernel` style booting.
  - Paging is enabled by the bootloader before it jumps to the kernel: the
    kernel runs with a fresh page table and the CR3 register set to the
    kernel's level-4 table.
  - The bootloader sets up the kernel stack and passes its top address;
    the kernel must not rely on a pre-existing stack.
  - Interrupts are disabled at kernel entry.
  - The bootloader may map the kernel at a dynamic address unless a fixed
    `kernel_base` is configured.
  - The `bootloader` crate is a development convenience; Elvaris may replace
    it with a custom bootloader later.