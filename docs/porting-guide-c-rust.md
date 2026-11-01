# Porting Guide: C and Rust to Elvaris

## C
- Target Elvaris native toolchain/SDK
- Use native syscalls or portable libraries
- Handle paths as absolute Elvaris paths
- Declare permissions in manifest

## Rust
- Prefer std if supported, else no_std + alloc
- Use elvaris-sdk for packaging
- Mind async runtime limitations

## Common
- Permissions must be declared
- Resources limited by policy
- No direct hardware access
