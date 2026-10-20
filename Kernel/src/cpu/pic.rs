//! The legacy 8259 interrupt controllers.
//!
//! The two programmable interrupt controllers are the oldest interrupt
//! controllers of the PC architecture. They are still present in the QEMU
//! machine the kernel runs on, and they are the reason the kernel has to
//! remap them: out of the box the master controller uses the vector range
//! 0x08..0x0F, which overlaps the CPU exception vectors.
//!
//! The kernel remaps the controllers to 0x20..0x2F and masks every line
//! except the one the timer uses. The local APIC and the I/O APIC are left
//! to a later milestone; see `docs/milestone-6.md` for the reasoning.

/// Command register of the master controller.
pub const PIC1_COMMAND: u16 = 0x20;

/// Data register of the master controller.
pub const PIC1_DATA: u16 = 0x21;

/// Command register of the slave controller.
pub const PIC2_COMMAND: u16 = 0xA0;

/// Data register of the slave controller.
pub const PIC2_DATA: u16 = 0xA1;

/// The end-of-interrupt command.
pub const PIC_EOI: u8 = 0x20;

/// The vector range of the master controller after remapping.
pub const PIC1_VECTOR_BASE: u8 = 0x20;

/// The vector range of the slave controller after remapping.
pub const PIC2_VECTOR_BASE: u8 = 0x28;

/// Initialization control word 1: expect initialization and a fourth word.
pub const ICW1_INIT: u8 = 0x11;

/// Initialization control word 4: 8086/88 mode.
pub const ICW4_8086: u8 = 0x01;

/// The state of the interrupt controllers after initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PicState {
    /// The vector base of the master controller.
    pub master_base: u8,
    /// The vector base of the slave controller.
    pub slave_base: u8,
    /// The interrupt mask of the master controller.
    pub master_mask: u8,
    /// The interrupt mask of the slave controller.
    pub slave_mask: u8,
    /// The interrupt mask of the master controller before initialization.
    pub previous_master_mask: u8,
    /// The interrupt mask of the slave controller before initialization.
    pub previous_slave_mask: u8,
}

/// Remap the interrupt controllers and mask every line.
///
/// The controllers are reinitialized with the master at 0x20 and the slave at
/// 0x28, so that no hardware interrupt can collide with a CPU exception.
/// Every line is masked afterwards; the kernel unmasks the timer line when
/// the timer is ready.
pub fn init() -> PicState {
    // Remember the current masks: the initialization sequence resets them.
    let saved_master = unsafe { read_port(PIC1_DATA) };
    let saved_slave = unsafe { read_port(PIC2_DATA) };

    // Initialization control words.
    unsafe {
        write_port(PIC1_COMMAND, ICW1_INIT);
        write_port(PIC2_COMMAND, ICW1_INIT);
    }
    unsafe {
        write_port(PIC1_DATA, PIC1_VECTOR_BASE);
        write_port(PIC2_DATA, PIC2_VECTOR_BASE);
    }
    unsafe {
        // The slave is connected to the master through interrupt line 2.
        write_port(PIC1_DATA, 0x04);
        write_port(PIC2_DATA, 0x02);
    }
    unsafe {
        write_port(PIC1_DATA, ICW4_8086);
        write_port(PIC2_DATA, ICW4_8086);
    }

    // Mask everything.
    unsafe {
        write_port(PIC1_DATA, 0xFF);
        write_port(PIC2_DATA, 0xFF);
    }

    PicState {
        master_base: PIC1_VECTOR_BASE,
        slave_base: PIC2_VECTOR_BASE,
        master_mask: 0xFF,
        slave_mask: 0xFF,
        previous_master_mask: saved_master,
        previous_slave_mask: saved_slave,
    }
}

/// Unmask a single interrupt line.
///
/// `irq` is the interrupt request number, not the vector.
pub fn unmask_irq(irq: u8) {
    if irq < 8 {
        let mask = unsafe { read_port(PIC1_DATA) } & !(1 << irq);
        unsafe { write_port(PIC1_DATA, mask) };
    } else {
        let mask = unsafe { read_port(PIC2_DATA) } & !(1 << (irq - 8));
        unsafe { write_port(PIC2_DATA, mask) };
    }
}

/// Mask every interrupt line again.
pub fn mask_all() {
    unsafe {
        write_port(PIC1_DATA, 0xFF);
        write_port(PIC2_DATA, 0xFF);
    }
}

/// The current interrupt mask of the master controller.
pub fn master_mask() -> u8 {
    unsafe { read_port(PIC1_DATA) }
}

/// The current interrupt mask of the slave controller.
pub fn slave_mask() -> u8 {
    unsafe { read_port(PIC2_DATA) }
}

/// Acknowledge an interrupt.
///
/// The master controller is always acknowledged; the slave controller is
/// acknowledged as well for the interrupts it owns.
pub fn end_of_interrupt(irq: u8) {
    if irq >= 8 {
        unsafe { write_port(PIC2_COMMAND, PIC_EOI) };
    }
    unsafe { write_port(PIC1_COMMAND, PIC_EOI) };
}

/// Handle a hardware interrupt.
///
/// The kernel has no device drivers yet, so the interrupt is only counted and
/// acknowledged.
pub fn handle_hardware_interrupt(irq: u8) {
    crate::cpu::timer::count_hardware_interrupt();
    end_of_interrupt(irq);
}

/// Read a byte from an I/O port.
///
/// # Safety
/// Port I/O is inherently unsafe.
pub unsafe fn read_port(port: u16) -> u8 {
    let value: u8;
    unsafe {
        core::arch::asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

/// Write a byte to an I/O port.
///
/// # Safety
/// Port I/O is inherently unsafe.
pub unsafe fn write_port(port: u16, value: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}
