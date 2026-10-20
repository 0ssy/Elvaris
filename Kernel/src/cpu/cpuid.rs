//! CPU feature identification.
//!
//! The kernel uses the CPU identification instruction to learn what the
//! processor supports. The local APIC, for example, is only present when the
//! feature bits say so.

/// The result of the `cpuid` instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuId {
    /// The first output word (EAX).
    pub eax: u32,
    /// The second output word (EBX).
    pub ebx: u32,
    /// The third output word (ECX).
    pub ecx: u32,
    /// The fourth output word (EDX).
    pub edx: u32,
}

/// Execute the CPU identification instruction.
///
/// The compiler reserves `rbx`, so the value the instruction writes there is
/// copied out before the register is restored.
pub fn cpuid(leaf: u32, subleaf: u32) -> CpuId {
    let mut result = [0u32; 4];
    let mut ebx: u32;
    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "mov r10d, ebx",
            "pop rbx",
            out("r10") ebx,
            inlateout("eax") leaf => result[0],
            inlateout("ecx") subleaf => result[2],
            inlateout("edx") result[3] => result[3],
        );
    }
    CpuId {
        eax: result[0],
        ebx,
        ecx: result[2],
        edx: result[3],
    }
}

/// The highest supported leaf of the CPU identification instruction.
pub fn max_leaf() -> u32 {
    cpuid(0, 0).eax
}

/// The vendor string of the processor.
pub fn vendor_string() -> [u8; 12] {
    let leaf = cpuid(0, 0);
    let mut vendor = [0u8; 12];
    vendor[0..4].copy_from_slice(&leaf.ebx.to_le_bytes());
    vendor[4..8].copy_from_slice(&leaf.edx.to_le_bytes());
    vendor[8..12].copy_from_slice(&leaf.ecx.to_le_bytes());
    vendor
}

/// The family, model and stepping of the processor.
pub fn family_model_stepping() -> (u8, u8, u8) {
    let leaf = cpuid(1, 0);
    let stepping = (leaf.eax & 0xF) as u8;
    let model = ((leaf.eax >> 4) & 0xF) as u8;
    let family = ((leaf.eax >> 8) & 0xF) as u8;
    (family, model, stepping)
}

/// Feature bits of the first identification leaf.
pub mod features {
    /// The processor reports its features in the second identification leaf.
    pub const LEAF1: u32 = 1;

    /// Bit 9 of leaf 1, EDX: a local APIC is present.
    pub const LOCAL_APIC: u32 = 1 << 9;
    /// Bit 5 of leaf 1, EDX: the time stamp counter is present.
    pub const TSC: u32 = 1 << 4;
    /// Bit 20 of leaf 1, EDX: the no-execute page protection is supported.
    pub const NX: u32 = 1 << 20;
    /// Bit 26 of leaf 1, EDX: the processor supports the SSE2 instruction set.
    pub const SSE2: u32 = 1 << 26;
    /// Bit 29 of leaf 1, EDX: the processor supports the hyper-threading feature.
    pub const HYPER_THREADING: u32 = 1 << 28;
}

/// Whether the local APIC is present.
pub fn has_local_apic() -> bool {
    cpuid(features::LEAF1, 0).edx & features::LOCAL_APIC != 0
}

/// Whether the time stamp counter is present.
pub fn has_tsc() -> bool {
    cpuid(features::LEAF1, 0).edx & features::TSC != 0
}

/// Whether the no-execute page protection is supported.
pub fn has_nx() -> bool {
    cpuid(features::LEAF1, 0).edx & features::NX != 0
}
