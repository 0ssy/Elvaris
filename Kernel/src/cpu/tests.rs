//! Unit tests for the pure parts of the CPU infrastructure.
//!
//! The tests cover the descriptor encodings, the vector metadata and the
//! layout of the captured register state. They run on the host, without any
//! hardware.

use core::mem::{offset_of, size_of};

use super::apic::{
    lvt_timer_value, DIVIDE_BY_16, LAPIC_BUS_FREQUENCY, LVT_TIMER_MASK, LVT_TIMER_RESET_MASK,
    TIMER_DIVIDE, TIMER_MODE_PERIODIC,
};
use super::descriptor::{
    code_segment, data_segment, idt_entry, make_selector, split_selector, table_descriptor,
    tss_descriptor, DATA_ACCESS, INTERRUPT_GATE, TRAP_GATE,
};
use super::exception::CPU_FRAME_OFFSET;
use super::pit::{divisor_for_frequency, frequency_for_divisor, PIT_FREQUENCY};
use super::vectors::{
    exception_name, has_error_code, is_exception, is_hardware_interrupt, is_reserved,
    page_fault, protection, vector, InterruptFrame, EXCEPTION_VECTOR_COUNT, HARDWARE_VECTOR_BASE,
    HARDWARE_VECTOR_LAST, TIMER_VECTOR,
};

#[test]
fn error_code_vectors_are_known() {
    // The processor pushes an error code for exactly these exceptions.
    for value in [
        vector::DOUBLE_FAULT,
        vector::INVALID_TSS,
        vector::SEGMENT_NOT_PRESENT,
        vector::STACK_SEGMENT_FAULT,
        vector::GENERAL_PROTECTION_FAULT,
        vector::PAGE_FAULT,
        vector::ALIGNMENT_CHECK,
        vector::CONTROL_PROTECTION_EXCEPTION,
    ] {
        assert!(has_error_code(value), "vector {} must carry an error code", value);
    }
    for value in [
        vector::DIVIDE_ERROR,
        vector::DEBUG,
        vector::BREAKPOINT,
        vector::INVALID_OPCODE,
        vector::DOUBLE_FAULT + 1,
        vector::MACHINE_CHECK,
        vector::SIMD_FLOATING_POINT,
    ] {
        assert!(
            !has_error_code(value),
            "vector {} must not carry an error code",
            value
        );
    }
}

#[test]
fn exception_names_cover_the_architectured_vectors() {
    assert_eq!(exception_name(vector::DIVIDE_ERROR), Some("Divide Error"));
    assert_eq!(exception_name(vector::BREAKPOINT), Some("Breakpoint"));
    assert_eq!(exception_name(vector::INVALID_OPCODE), Some("Invalid Opcode"));
    assert_eq!(exception_name(vector::DOUBLE_FAULT), Some("Double Fault"));
    assert_eq!(exception_name(vector::INVALID_TSS), Some("Invalid TSS"));
    assert_eq!(
        exception_name(vector::GENERAL_PROTECTION_FAULT),
        Some("General Protection Fault")
    );
    assert_eq!(exception_name(vector::PAGE_FAULT), Some("Page Fault"));
    assert_eq!(exception_name(vector::MACHINE_CHECK), Some("Machine Check"));
    // Vectors outside the exception range have no name.
    assert_eq!(exception_name(HARDWARE_VECTOR_BASE), None);
    assert_eq!(exception_name(TIMER_VECTOR), None);
}

#[test]
fn reserved_vectors_are_identified() {
    assert!(is_reserved(9));
    assert!(is_reserved(15));
    assert!(is_reserved(22));
    assert!(is_reserved(31));
    assert!(!is_reserved(vector::PAGE_FAULT));
    assert!(!is_reserved(vector::MACHINE_CHECK));
}

#[test]
fn vector_ranges_do_not_overlap() {
    assert!(is_exception(vector::DIVIDE_ERROR));
    assert!(!is_exception(HARDWARE_VECTOR_BASE));
    assert!(is_hardware_interrupt(HARDWARE_VECTOR_BASE));
    assert!(is_hardware_interrupt(HARDWARE_VECTOR_LAST));
    assert!(!is_hardware_interrupt(HARDWARE_VECTOR_LAST + 1));
    assert!(!is_hardware_interrupt(EXCEPTION_VECTOR_COUNT - 1));
    // The timer vector is outside both ranges.
    assert!(!is_exception(TIMER_VECTOR));
    assert!(!is_hardware_interrupt(TIMER_VECTOR));
}

#[test]
fn kernel_code_segment_has_the_expected_encoding() {
    // 0x00AF9A000000FFFF: present, ring 0, 64-bit code, base 0, limit 4 GiB.
    assert_eq!(code_segment(0, 0xFFFF_FFFF, 0), 0x00AF_9A00_0000_FFFF);
}

#[test]
fn kernel_data_segment_has_the_expected_encoding() {
    // 0x00CF92000000FFFF: present, ring 0, writable, base 0, limit 4 GiB.
    assert_eq!(data_segment(0, 0xFFFF_FFFF, 0), 0x00CF_9200_0000_FFFF);
}

#[test]
fn user_segments_carry_the_requested_privilege_level() {
    // Ring 3 sets the two privilege-level bits of the access byte.
    assert_eq!(code_segment(0, 0xFFFF_FFFF, 3), 0x00AF_FA00_0000_FFFF);
    assert_eq!(data_segment(0, 0xFFFF_FFFF, 3), 0x00CF_F200_0000_FFFF);
    // The data access byte stays writable.
    assert_eq!(data_segment(0, 0xFFFF_FFFF, 3) >> 40 & 0xFF, 0xF2);
}

#[test]
fn segment_base_and_limit_are_recoverable() {
    let descriptor = data_segment(0x1234_5678, 0x000F_FFFF, 0);
    let base = ((descriptor >> 16) & 0xFFFF)
        | (((descriptor >> 32) & 0xFF) << 16)
        | (((descriptor >> 56) & 0xFF) << 24);
    let limit = (descriptor & 0xFFFF) | (((descriptor >> 48) & 0xF) << 16);
    assert_eq!(base, 0x1234_5678);
    assert_eq!(limit, 0x000F_FFFF);
}

#[test]
fn access_byte_of_a_data_segment_is_writable() {
    // The writable bit is the second bit of the type field, which lives in
    // the low four bits of the access byte.
    assert_eq!(DATA_ACCESS & 0b0000_0010, 0b0000_0010);
    // The access byte is present, a data segment, and ring 0.
    assert_eq!(DATA_ACCESS & 0b1000_0000, 0b1000_0000);
    assert_eq!(DATA_ACCESS & 0b0001_0000, 0b0001_0000);
    assert_eq!(DATA_ACCESS >> 5 & 0b11, 0);
}

#[test]
fn tss_descriptor_holds_base_and_limit() {
    let descriptor = tss_descriptor(0x0000_1234_5678_9ABC, 0x67);
    let limit = (descriptor & 0xFFFF) | (((descriptor >> 48) & 0xF) << 16);
    let base = ((descriptor >> 16) & 0xFF_FFFF)
        | (((descriptor >> 56) & 0xFF) << 24)
        | (((descriptor >> 64) & 0xFFFF_FFFF) << 32);
    assert_eq!(limit, 0x67);
    assert_eq!(base, 0x0000_1234_5678_9ABC);
    // The type field of a 64-bit available task state segment is 0b1001.
    assert_eq!((descriptor >> 40) & 0xF, 0b1001);
    // The descriptor has to be present, otherwise the processor refuses to
    // load the segment.
    assert_eq!((descriptor >> 47) & 1, 1);
    // The bits the descriptor does not use stay clear.
    assert_eq!((descriptor >> 52) & 0xF, 0);
    assert_eq!((descriptor >> 96) & 0xFFFF_FFFF, 0);
}

#[test]
fn idt_entry_holds_offset_selector_and_ist() {
    let entry = idt_entry(0x0000_1234_5678_9ABC, 0x08, 1, INTERRUPT_GATE);
    let offset = (entry & 0xFFFF)
        | (((entry >> 48) & 0xFFFF) << 16)
        | (((entry >> 64) & 0xFFFF_FFFF) << 32);
    assert_eq!(offset, 0x0000_1234_5678_9ABC);
    assert_eq!((entry >> 16) & 0xFFFF, 0x08);
    assert_eq!((entry >> 32) & 0x7, 1);
    // Present, ring 0, 64-bit interrupt gate.
    assert_eq!((entry >> 40) & 0xFF, 0x8E);
    // The trap gate differs in the gate type only.
    let trap = idt_entry(0x1000, 0x08, 0, TRAP_GATE);
    assert_eq!((trap >> 40) & 0xFF, 0x8F);
}

#[test]
fn table_descriptor_holds_limit_and_base() {
    let descriptor = table_descriptor(0x0000_0000_1234_5678, 0x0FFF);
    assert_eq!(descriptor[0], 0xFF);
    assert_eq!(descriptor[1], 0x0F);
    let mut base = [0u8; 8];
    base.copy_from_slice(&descriptor[2..10]);
    assert_eq!(u64::from_le_bytes(base), 0x0000_0000_1234_5678);
}

#[test]
fn selectors_carry_index_and_privilege_level() {
    let (index, rpl, table) = split_selector(0x1B);
    assert_eq!(index, 0x18);
    assert_eq!(rpl, 3);
    assert!(!table);
    assert_eq!(make_selector(0x18, 3), 0x1B);
    let (_, _, ldt) = split_selector(0x04 | 0x3);
    assert!(ldt);
}

#[test]
fn page_fault_error_code_bits_are_distinct() {
    assert_eq!(page_fault::PRESENT, 1 << 0);
    assert_eq!(page_fault::WRITE, 1 << 1);
    assert_eq!(page_fault::USER, 1 << 2);
    assert_eq!(page_fault::RESERVED_WRITE, 1 << 3);
    assert_eq!(page_fault::INSTRUCTION_FETCH, 1 << 4);
    assert_eq!(page_fault::SHADOW_STACK, 1 << 5);
    // A protection violation sets the present bit and reports a write.
    let code = page_fault::PRESENT | page_fault::WRITE;
    assert_eq!(code & page_fault::PRESENT, page_fault::PRESENT);
    assert_eq!(code & page_fault::WRITE, page_fault::WRITE);
    assert_eq!(code & page_fault::USER, 0);
}

#[test]
fn protection_error_code_is_decoded() {
    // A general protection fault caused by the selector 0x10 of the global
    // descriptor table.
    let error_code = 0x10;
    assert_eq!(protection::selector_index(error_code), 0x10);
    assert_eq!(protection::table(error_code), protection::Table::Gdt);
    assert_eq!(error_code & protection::EXTERNAL, 0);
    assert_eq!(error_code & protection::IDT, 0);

    // An external event that referenced the interrupt descriptor table.
    let external = protection::EXTERNAL | protection::IDT | 0x20;
    assert_eq!(protection::selector_index(external), 0x20);
    assert_eq!(protection::table(external), protection::Table::Idt);
    assert_ne!(external & protection::EXTERNAL, 0);
}

#[test]
fn interrupt_frame_has_the_expected_layout() {
    // The stub pushes fifteen registers, the vector and the placeholder
    // before the processor's own frame. In 64-bit mode the processor always
    // pushes the instruction pointer, the code segment, the flags, the stack
    // pointer and the stack segment.
    assert_eq!(size_of::<InterruptFrame>(), 176);
    assert_eq!(offset_of!(InterruptFrame, r15), 0);
    assert_eq!(offset_of!(InterruptFrame, rax), 112);
    assert_eq!(offset_of!(InterruptFrame, vector), 120);
    assert_eq!(offset_of!(InterruptFrame, error_code), 128);
    assert_eq!(offset_of!(InterruptFrame, rip), 136);
    assert_eq!(offset_of!(InterruptFrame, cs), 144);
    assert_eq!(offset_of!(InterruptFrame, rflags), 152);
    assert_eq!(offset_of!(InterruptFrame, rsp), 160);
    assert_eq!(offset_of!(InterruptFrame, ss), 168);
    assert_eq!(CPU_FRAME_OFFSET, 136);
}

#[test]
fn interrupt_frame_reports_its_vector_and_error_code() {
    let frame = InterruptFrame {
        r15: 0,
        r14: 0,
        r13: 0,
        r12: 0,
        r11: 0,
        r10: 0,
        r9: 0,
        r8: 0,
        rdi: 0,
        rsi: 0,
        rbp: 0,
        rbx: 0,
        rdx: 0,
        rcx: 0,
        rax: 0,
        vector: vector::PAGE_FAULT as u64,
        error_code: 0x2,
        rip: 0x1000,
        cs: 0x08,
        rflags: 0x202,
        rsp: 0x2000,
        ss: 0x10,
    };
    assert_eq!(frame.vector(), vector::PAGE_FAULT);
    assert_eq!(frame.error_code(), 0x2);
    assert_eq!(frame.rip(), 0x1000);
    assert_eq!(frame.rsp(), 0x2000);
    assert_eq!(frame.cs(), 0x08);
    assert_eq!(frame.rflags(), 0x202);
    assert!(!frame.from_user_mode());
}

#[test]
fn user_mode_events_are_recognized() {
    // The code segment of the interrupted code carries the privilege level in
    // its two lowest bits.
    let mut frame = InterruptFrame {
        r15: 0,
        r14: 0,
        r13: 0,
        r12: 0,
        r11: 0,
        r10: 0,
        r9: 0,
        r8: 0,
        rdi: 0,
        rsi: 0,
        rbp: 0,
        rbx: 0,
        rdx: 0,
        rcx: 0,
        rax: 0,
        vector: vector::PAGE_FAULT as u64,
        error_code: 0x2,
        rip: 0x1000,
        cs: 0x08,
        rflags: 0x202,
        rsp: 0x2000,
        ss: 0x10,
    };
    assert!(!frame.from_user_mode());

    // A code segment with the privilege level three belongs to user mode.
    frame.cs = 0x1B;
    assert!(frame.from_user_mode());
}

#[test]
fn apic_timer_entry_selects_the_periodic_mode() {
    let value = lvt_timer_value(TIMER_VECTOR, TIMER_MODE_PERIODIC);
    // The vector lives in the low byte.
    assert_eq!(value & 0xFF, TIMER_VECTOR as u32);
    // The mode bits hold the periodic mode.
    assert_eq!((value >> 17) & 0b11, 0b01);
    // The mask bit stays clear, otherwise the timer never interrupts.
    assert_eq!(value & LVT_TIMER_MASK, 0);
    // The mask bit covers the bit the processor uses to disable the timer.
    assert_eq!(LVT_TIMER_MASK, 1 << 16);
    assert_eq!(LVT_TIMER_RESET_MASK & LVT_TIMER_MASK, LVT_TIMER_MASK);
    assert_eq!(LVT_TIMER_RESET_MASK & (0b11 << 17), 0b11 << 17);
}

#[test]
fn apic_timer_divider_is_applied() {
    // A divide factor of 16 leaves a bus frequency of 6.25 MHz.
    assert_eq!(LAPIC_BUS_FREQUENCY / TIMER_DIVIDE, 6_250_000);
    // A hundred interrupts per second need 62500 counts.
    assert_eq!(LAPIC_BUS_FREQUENCY / TIMER_DIVIDE / 100, 62_500);
    // The divisor value that selects a divide factor of 16.
    assert_eq!(DIVIDE_BY_16, 0b0011);
}

#[test]
fn pit_divisor_matches_the_requested_frequency() {
    let divisor = divisor_for_frequency(100).expect("100 Hz is reachable");
    assert_eq!(divisor, 11932);
    // The frequency that results from the divisor is close to the request.
    let frequency = frequency_for_divisor(divisor);
    assert!(frequency >= 99 && frequency <= 101);
    assert_eq!(frequency_for_divisor(0), 0);
    // A frequency the timer cannot produce has no divisor.
    assert_eq!(divisor_for_frequency(0), None);
    assert_eq!(divisor_for_frequency(1), None);
    assert!(divisor_for_frequency(1_000_000).is_some());
}

#[test]
fn pit_frequency_is_the_documented_one() {
    assert_eq!(PIT_FREQUENCY, 1_193_182);
}
