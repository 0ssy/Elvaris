//! The state of the processor.
//!
//! The kernel reports the state it inherited from the bootloader and the
//! state it has installed itself. The report is the starting point for
//! `docs/milestone-6.md`, which describes what the bootloader leaves behind
//! and what the kernel changes.

use crate::cpu::{apic, cpuid, gdt, idt, interrupts, pic, tss};
use crate::serial;

/// The model specific register that holds the extended feature flags.
const MSR_EFER: u32 = 0xC000_0080;

/// A snapshot of the interesting processor state.
#[derive(Debug, Clone, Copy)]
pub struct CpuState {
    /// The control register 0.
    pub cr0: u64,
    /// The control register 2: the address of the last page fault.
    pub cr2: u64,
    /// The control register 3: the base of the page tables.
    pub cr3: u64,
    /// The control register 4.
    pub cr4: u64,
    /// The extended feature flags.
    pub efer: u64,
    /// The base of the global descriptor table.
    pub gdt_base: u64,
    /// The limit of the global descriptor table.
    pub gdt_limit: u16,
    /// The base of the interrupt descriptor table.
    pub idt_base: u64,
    /// The limit of the interrupt descriptor table.
    pub idt_limit: u16,
    /// The selector of the task state segment.
    pub tr_selector: u16,
    /// Whether interrupts are enabled.
    pub interrupts_enabled: bool,
    /// Whether the processor reports a local APIC.
    pub apic_present: bool,
    /// Whether the local APIC is enabled.
    pub apic_enabled: bool,
    /// The identifier of the local APIC.
    pub apic_id: u32,
    /// The interrupt mask of the master interrupt controller.
    pub pic_master_mask: u8,
    /// The interrupt mask of the slave interrupt controller.
    pub pic_slave_mask: u8,
}

/// Read control register 0.
pub fn read_cr0() -> u64 {
    let value: u64;
    unsafe {
        core::arch::asm!("mov {}, cr0", out(reg) value, options(nomem, nostack));
    }
    value
}

/// Read control register 2.
pub fn read_cr2() -> u64 {
    let value: u64;
    unsafe {
        core::arch::asm!("mov {}, cr2", out(reg) value, options(nomem, nostack));
    }
    value
}

/// Read control register 3.
pub fn read_cr3() -> u64 {
    let value: u64;
    unsafe {
        core::arch::asm!("mov {}, cr3", out(reg) value, options(nomem, nostack));
    }
    value
}

/// Read control register 4.
pub fn read_cr4() -> u64 {
    let value: u64;
    unsafe {
        core::arch::asm!("mov {}, cr4", out(reg) value, options(nomem, nostack));
    }
    value
}

/// Read the extended feature flags.
pub fn read_efer() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") MSR_EFER,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack)
        );
    }
    ((high as u64) << 32) | low as u64
}

/// Read the global descriptor table register.
pub fn read_gdtr() -> (u64, u16) {
    let mut descriptor = [0u8; 10];
    unsafe {
        core::arch::asm!("sgdt [{}]", in(reg) &mut descriptor, options(nostack));
    }
    let limit = u16::from_le_bytes([descriptor[0], descriptor[1]]);
    let mut base = [0u8; 8];
    base.copy_from_slice(&descriptor[2..10]);
    (u64::from_le_bytes(base), limit)
}

/// Read the interrupt descriptor table register.
pub fn read_idtr() -> (u64, u16) {
    let mut descriptor = [0u8; 10];
    unsafe {
        core::arch::asm!("sidt [{}]", in(reg) &mut descriptor, options(nostack));
    }
    let limit = u16::from_le_bytes([descriptor[0], descriptor[1]]);
    let mut base = [0u8; 8];
    base.copy_from_slice(&descriptor[2..10]);
    (u64::from_le_bytes(base), limit)
}

/// Collect the current state of the processor.
pub fn snapshot() -> CpuState {
    let cr0 = read_cr0();
    let cr2 = read_cr2();
    let cr3 = read_cr3();
    let cr4 = read_cr4();
    let efer = read_efer();
    let (gdt_base, gdt_limit) = read_gdtr();
    let (idt_base, idt_limit) = read_idtr();
    let tr_selector = tss::task_register_selector();
    let interrupts_enabled = interrupts::are_enabled();
    let apic_present = apic::is_present();
    let apic_enabled = apic::is_enabled();
    let apic_id = apic::id();
    let pic_master_mask = pic::master_mask();
    let pic_slave_mask = pic::slave_mask();
    CpuState {
        cr0,
        cr2,
        cr3,
        cr4,
        efer,
        gdt_base,
        gdt_limit,
        idt_base,
        idt_limit,
        tr_selector,
        interrupts_enabled,
        apic_present,
        apic_enabled,
        apic_id,
        pic_master_mask,
        pic_slave_mask,
    }
}

/// Print the state of the processor.
pub fn report() {
    let state = snapshot();
    let vendor = cpuid::vendor_string();
    let (family, model, stepping) = cpuid::family_model_stepping();

    unsafe {
        serial::write_str("Elvaris kernel: --- cpu state ---\r\n");
        serial::write_str("Elvaris kernel: cpu: vendor=");
        serial::write_bytes(&vendor);
        serial::write_str(" family=");
        serial::write_fmt(core::format_args!("{}", family));
        serial::write_str(" model=");
        serial::write_fmt(core::format_args!("{}", model));
        serial::write_str(" stepping=");
        serial::write_fmt(core::format_args!("{}", stepping));
        serial::write_newline();
        serial::write_str("Elvaris kernel: cpu: cr0=0x");
        serial::write_fmt(core::format_args!("{:x}", state.cr0));
        serial::write_str(" cr2=0x");
        serial::write_fmt(core::format_args!("{:x}", state.cr2));
        serial::write_str(" cr3=0x");
        serial::write_fmt(core::format_args!("{:x}", state.cr3));
        serial::write_str(" cr4=0x");
        serial::write_fmt(core::format_args!("{:x}", state.cr4));
        serial::write_newline();
        serial::write_str("Elvaris kernel: cpu: efer=0x");
        serial::write_fmt(core::format_args!("{:x}", state.efer));
        serial::write_str(" nx=");
        serial::write_fmt(core::format_args!("{}", state.efer & (1 << 11) != 0));
        serial::write_newline();
        serial::write_str("Elvaris kernel: cpu: gdt base=0x");
        serial::write_fmt(core::format_args!("{:x}", state.gdt_base));
        serial::write_str(" limit=");
        serial::write_fmt(core::format_args!("{}", state.gdt_limit));
        serial::write_str(" idt base=0x");
        serial::write_fmt(core::format_args!("{:x}", state.idt_base));
        serial::write_str(" limit=");
        serial::write_fmt(core::format_args!("{}", state.idt_limit));
        serial::write_str(" tr=0x");
        serial::write_fmt(core::format_args!("{:x}", state.tr_selector));
        serial::write_newline();
        serial::write_str("Elvaris kernel: cpu: interrupts_enabled=");
        serial::write_fmt(core::format_args!("{}", state.interrupts_enabled));
        serial::write_str(" apic_present=");
        serial::write_fmt(core::format_args!("{}", state.apic_present));
        serial::write_str(" apic_enabled=");
        serial::write_fmt(core::format_args!("{}", state.apic_enabled));
        serial::write_str(" apic_id=");
        serial::write_fmt(core::format_args!("{}", state.apic_id));
        serial::write_newline();
        serial::write_str("Elvaris kernel: cpu: pic master_mask=0x");
        serial::write_fmt(core::format_args!("{:x}", state.pic_master_mask));
        serial::write_str(" slave_mask=0x");
        serial::write_fmt(core::format_args!("{:x}", state.pic_slave_mask));
        serial::write_newline();
        serial::write_str("Elvaris kernel: --- end cpu state ---\r\n");
    }
}

/// The base address of the installed global descriptor table.
pub fn installed_gdt_base() -> u64 {
    gdt::base_address()
}

/// The limit of the installed global descriptor table.
pub fn installed_gdt_limit() -> u16 {
    gdt::limit()
}

/// The base address of the installed interrupt descriptor table.
pub fn installed_idt_base() -> u64 {
    idt::base_address()
}

/// The limit of the installed interrupt descriptor table.
pub fn installed_idt_limit() -> u16 {
    idt::limit()
}
