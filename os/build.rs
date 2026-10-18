use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());

    // Path to the compiled kernel ELF, provided by Cargo's artifact dependency.
    let kernel = PathBuf::from(std::env::var_os("CARGO_BIN_FILE_KERNEL_kernel").unwrap());

    // Build the UEFI GPT disk image.
    let uefi_path = out_dir.join("uefi.img");
    bootloader::UefiBoot::new(&kernel)
        .create_disk_image(&uefi_path)
        .expect("failed to create UEFI disk image");

    // Expose the image path to the host binary.
    println!("cargo:rustc-env=UEFI_PATH={}", uefi_path.display());
    println!("cargo:rerun-if-changed=kernel");
}