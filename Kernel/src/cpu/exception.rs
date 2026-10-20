//! Exception handling.
//!
//! Every event the processor delivers — a CPU exception, a hardware
//! interrupt or the kernel timer — enters the kernel through one of the
//! interrupt stubs. The stub saves the register state, calls the common
//! entry point, and restores the register state when the handler returns.
//!
//! A fatal exception never returns: it prints the diagnostic and terminates
//! the machine through the development-only QEMU exit device.

use crate::cpu::gdt::KERNEL_CODE_SELECTOR;
use crate::cpu::tss::DOUBLE_FAULT_IST;
use crate::cpu::vectors::{
    exception_name, has_error_code, is_exception, is_hardware_interrupt, InterruptFrame,
    TIMER_VECTOR,
};
use crate::exit;
use crate::serial;

/// Offset of the processor's own frame, relative to the address of the frame.
///
/// The stub pushes fifteen general-purpose registers (120 bytes), the vector
/// (8 bytes) and the placeholder for the error code (8 bytes). The processor
/// pushes the instruction pointer, the code segment, the flags, the stack
/// pointer and the stack segment above them; in 64-bit mode it always pushes
/// the stack pointer and the stack segment.
pub const CPU_FRAME_OFFSET: u64 = 136;

/// Generate the interrupt stubs.
///
/// Every stub has the same shape: save the registers, push the vector, call
/// the common entry point, restore the registers and return to the interrupted
/// code. Vectors that carry an error code get the error code from the
/// processor; the others push a placeholder so that the captured register
/// state has the same layout for every vector.
macro_rules! define_stubs {
    ($(($vec:literal, $placeholder:literal)),* $(,)?) => {
        $(
            core::arch::global_asm!(concat!(
                ".section .text\n",
                ".globl isr_stub_", $vec, "\n",
                "isr_stub_", $vec, ":\n",
                $placeholder,
                "push ", $vec, "\n",
                "push rax\n",
                "push rcx\n",
                "push rdx\n",
                "push rbx\n",
                "push rbp\n",
                "push rsi\n",
                "push rdi\n",
                "push r8\n",
                "push r9\n",
                "push r10\n",
                "push r11\n",
                "push r12\n",
                "push r13\n",
                "push r14\n",
                "push r15\n",
                "mov rdi, rsp\n",
                "call elvaris_exception_entry\n",
                "pop r15\n",
                "pop r14\n",
                "pop r13\n",
                "pop r12\n",
                "pop r11\n",
                "pop r10\n",
                "pop r9\n",
                "pop r8\n",
                "pop rdi\n",
                "pop rsi\n",
                "pop rbp\n",
                "pop rbx\n",
                "pop rdx\n",
                "pop rcx\n",
                "pop rax\n",
                // Skip the placeholder and the vector, so that the processor's
                // own frame is on the top of the stack again.
                "add rsp, 16\n",
                "iretq\n",
            ));
        )*
    };
}

define_stubs! {
    // CPU exceptions. The second field is the placeholder that is pushed for
    // the vectors that do not carry an error code.
    (0, "push 0\n"),    // #DE  divide error
    (1, "push 0\n"),    // #DB  debug
    (2, "push 0\n"),    // #NM  non-maskable interrupt
    (3, "push 0\n"),    // #BP  breakpoint
    (4, "push 0\n"),    // #OF  overflow
    (5, "push 0\n"),    // #BR  BOUND range exceeded
    (6, "push 0\n"),    // #UD  invalid opcode
    (7, "push 0\n"),    // #DN  device not available
    (8, ""),            // #DF  double fault (error code)
    (9, "push 0\n"),    // #CS  coprocessor segment override (reserved)
    (10, ""),           // #TS  invalid TSS (error code)
    (11, ""),           // #NP  segment not present (error code)
    (12, ""),           // #SS  stack-segment fault (error code)
    (13, ""),           // #GP  general protection fault (error code)
    (14, ""),           // #PF  page fault (error code)
    (15, "push 0\n"),   // reserved
    (16, "push 0\n"),   // #MF  x87 floating-point exception
    (17, ""),           // #AC  alignment check (error code)
    (18, "push 0\n"),   // #MC  machine check
    (19, "push 0\n"),   // #XM  SIMD floating-point exception
    (20, "push 0\n"),   // #VE  virtualization exception
    (21, ""),           // #CP  control protection exception (error code)
    (22, "push 0\n"),   // reserved
    (23, "push 0\n"),   // reserved
    (24, "push 0\n"),   // reserved
    (25, "push 0\n"),   // reserved
    (26, "push 0\n"),   // reserved
    (27, "push 0\n"),   // reserved
    (28, "push 0\n"),   // reserved
    (29, "push 0\n"),   // reserved
    (30, "push 0\n"),   // reserved
    (31, "push 0\n"),   // reserved
    // Hardware interrupts of the legacy interrupt controllers (IRQ0..IRQ15).
    (32, "push 0\n"),
    (33, "push 0\n"),
    (34, "push 0\n"),
    (35, "push 0\n"),
    (36, "push 0\n"),
    (37, "push 0\n"),
    (38, "push 0\n"),
    (39, "push 0\n"),
    (40, "push 0\n"),
    (41, "push 0\n"),
    (42, "push 0\n"),
    (43, "push 0\n"),
    (44, "push 0\n"),
    (45, "push 0\n"),
    (46, "push 0\n"),
    (47, "push 0\n"),
    // The kernel timer.
    (64, "push 0\n"),
    // The system call of the user mode.
    (128, "push 0\n"),
    // The spurious interrupt of the local APIC.
    (255, "push 0\n"),
}

unsafe extern "C" {
    fn isr_stub_0();
    fn isr_stub_1();
    fn isr_stub_2();
    fn isr_stub_3();
    fn isr_stub_4();
    fn isr_stub_5();
    fn isr_stub_6();
    fn isr_stub_7();
    fn isr_stub_8();
    fn isr_stub_9();
    fn isr_stub_10();
    fn isr_stub_11();
    fn isr_stub_12();
    fn isr_stub_13();
    fn isr_stub_14();
    fn isr_stub_15();
    fn isr_stub_16();
    fn isr_stub_17();
    fn isr_stub_18();
    fn isr_stub_19();
    fn isr_stub_20();
    fn isr_stub_21();
    fn isr_stub_22();
    fn isr_stub_23();
    fn isr_stub_24();
    fn isr_stub_25();
    fn isr_stub_26();
    fn isr_stub_27();
    fn isr_stub_28();
    fn isr_stub_29();
    fn isr_stub_30();
    fn isr_stub_31();
    fn isr_stub_32();
    fn isr_stub_33();
    fn isr_stub_34();
    fn isr_stub_35();
    fn isr_stub_36();
    fn isr_stub_37();
    fn isr_stub_38();
    fn isr_stub_39();
    fn isr_stub_40();
    fn isr_stub_41();
    fn isr_stub_42();
    fn isr_stub_43();
    fn isr_stub_44();
    fn isr_stub_45();
    fn isr_stub_46();
    fn isr_stub_47();
    fn isr_stub_64();
    fn isr_stub_128();
    fn isr_stub_255();
}

/// The stubs of the CPU exceptions, indexed by vector.
pub static EXCEPTION_STUBS: [unsafe extern "C" fn(); 32] = [
    isr_stub_0,
    isr_stub_1,
    isr_stub_2,
    isr_stub_3,
    isr_stub_4,
    isr_stub_5,
    isr_stub_6,
    isr_stub_7,
    isr_stub_8,
    isr_stub_9,
    isr_stub_10,
    isr_stub_11,
    isr_stub_12,
    isr_stub_13,
    isr_stub_14,
    isr_stub_15,
    isr_stub_16,
    isr_stub_17,
    isr_stub_18,
    isr_stub_19,
    isr_stub_20,
    isr_stub_21,
    isr_stub_22,
    isr_stub_23,
    isr_stub_24,
    isr_stub_25,
    isr_stub_26,
    isr_stub_27,
    isr_stub_28,
    isr_stub_29,
    isr_stub_30,
    isr_stub_31,
];

/// The stubs of the legacy hardware interrupts, indexed by IRQ number.
pub static HARDWARE_STUBS: [unsafe extern "C" fn(); 16] = [
    isr_stub_32,
    isr_stub_33,
    isr_stub_34,
    isr_stub_35,
    isr_stub_36,
    isr_stub_37,
    isr_stub_38,
    isr_stub_39,
    isr_stub_40,
    isr_stub_41,
    isr_stub_42,
    isr_stub_43,
    isr_stub_44,
    isr_stub_45,
    isr_stub_46,
    isr_stub_47,
];

/// The stub of the kernel timer.
pub static TIMER_STUB: unsafe extern "C" fn() = isr_stub_64;

/// The stub of the system call of the user mode.
pub static SYSCALL_STUB: unsafe extern "C" fn() = isr_stub_128;

/// The stub of the spurious interrupt of the local APIC.
pub static SPURIOUS_STUB: unsafe extern "C" fn() = isr_stub_255;

/// The common entry point of every event.
///
/// The handler dispatches on the vector. Fatal exceptions do not return; the
/// timer tick and the hardware interrupts return, and the stub restores the
/// interrupted state.
///
/// The system call is the one event a user program can raise. It returns to
/// the user program like an ordinary interrupt does, unless the program
/// ended, in which case the handler resumes the kernel instead.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn elvaris_exception_entry(frame: *mut InterruptFrame) {
    // SAFETY: the stub passes a pointer to the frame it built on the
    // interrupt stack. The handler may write the return value of a system
    // call back into it; the stub restores the registers from the same
    // memory.
    let frame = unsafe { &mut *frame };
    let vector = frame.vector();

    if is_exception(vector) {
        fatal_exception(frame, vector);
    } else if vector == crate::cpu::vectors::SYSCALL_VECTOR {
        crate::user::syscall::handle(frame);
    } else if is_hardware_interrupt(vector) {
        crate::cpu::pic::handle_hardware_interrupt(
            vector - crate::cpu::vectors::HARDWARE_VECTOR_BASE,
        );
    } else if vector == TIMER_VECTOR {
        crate::cpu::timer::handle_timer_interrupt();
    } else if vector == crate::cpu::apic::SPURIOUS_VECTOR {
        crate::cpu::timer::handle_spurious_interrupt();
    } else {
        unexpected_vector(frame, vector);
    }
}

/// The values a fatal-exception diagnostic reports.
struct ExceptionReport {
    name: &'static str,
    vector: u8,
    error_code: u64,
    has_error_code: bool,
    rip: u64,
    rsp: u64,
    cs: u64,
    rflags: u64,
    fault_address: Option<u64>,
    registers: [u64; 8],
}

/// Report a fatal CPU exception and terminate the machine.
fn fatal_exception(frame: &InterruptFrame, vector: u8) -> ! {
    if frame.from_user_mode() {
        if crate::user::session::is_active() {
            // The exception came from the user program. It does not take the
            // kernel down: the user program is terminated instead, and the
            // kernel resumes where it left user mode.
            crate::user::session::handle_fault(frame, vector);
        }
        report_user_mode_exception(frame, vector);
    }

    let fault_address = if vector == crate::cpu::vectors::vector::PAGE_FAULT {
        Some(read_cr2())
    } else {
        None
    };
    let stack_pointer = interrupted_stack_pointer(frame);
    let report = ExceptionReport {
        name: exception_name(vector).unwrap_or("Unknown Exception"),
        vector,
        error_code: frame.error_code(),
        has_error_code: has_error_code(vector),
        rip: frame.rip(),
        rsp: stack_pointer,
        cs: frame.cs(),
        rflags: frame.rflags(),
        fault_address,
        registers: [
            frame.rax, frame.rbx, frame.rcx, frame.rdx, frame.rsi, frame.rdi, frame.rbp,
            stack_pointer,
        ],
    };
    print_report(&report);
    terminate()
}

/// Report an exception that arrived from user mode without a user program.
///
/// The kernel runs user code only while a user program is active. An
/// exception from user mode outside that window means that something in the
/// kernel switched the privilege level without preparing for it, and the
/// kernel cannot continue. The report says so instead of pretending that the
/// frame belongs to the kernel.
fn report_user_mode_exception(frame: &InterruptFrame, vector: u8) -> ! {
    unsafe {
        serial::write_str("Elvaris kernel: EXCEPTION: Unexpected User Mode Event\r\n");
        serial::write_str("Elvaris kernel: exception: vector=");
        serial::write_fmt(core::format_args!("{}", vector));
        serial::write_str(" user_stack_pointer=0x");
        serial::write_fmt(core::format_args!("{:x}", frame.rsp));
        serial::write_newline();
    }
    terminate()
}

/// Print the diagnostic of a fatal exception.
fn print_report(report: &ExceptionReport) {
    unsafe {
        serial::write_str("Elvaris kernel: EXCEPTION: ");
        serial::write_str(report.name);
        serial::write_newline();
        serial::write_str("Elvaris kernel: exception: vector=");
        serial::write_fmt(core::format_args!("{}", report.vector));
        if report.has_error_code {
            serial::write_str(" error_code=0x");
            serial::write_fmt(core::format_args!("{:x}", report.error_code));
        }
        serial::write_newline();
        serial::write_str("Elvaris kernel: exception: rip=0x");
        serial::write_fmt(core::format_args!("{:x}", report.rip));
        serial::write_str(" rsp=0x");
        serial::write_fmt(core::format_args!("{:x}", report.rsp));
        serial::write_str(" cs=0x");
        serial::write_fmt(core::format_args!("{:x}", report.cs));
        serial::write_newline();
        if let Some(address) = report.fault_address {
            serial::write_str("Elvaris kernel: exception: fault_address=0x");
            serial::write_fmt(core::format_args!("{:x}", address));
            serial::write_newline();
        }
        serial::write_str("Elvaris kernel: exception: rflags=0x");
        serial::write_fmt(core::format_args!("{:x}", report.rflags));
        serial::write_str(" rax=0x");
        serial::write_fmt(core::format_args!("{:x}", report.registers[0]));
        serial::write_str(" rbx=0x");
        serial::write_fmt(core::format_args!("{:x}", report.registers[1]));
        serial::write_str(" rcx=0x");
        serial::write_fmt(core::format_args!("{:x}", report.registers[2]));
        serial::write_str(" rdx=0x");
        serial::write_fmt(core::format_args!("{:x}", report.registers[3]));
        serial::write_newline();
        serial::write_str("Elvaris kernel: exception: rsi=0x");
        serial::write_fmt(core::format_args!("{:x}", report.registers[4]));
        serial::write_str(" rdi=0x");
        serial::write_fmt(core::format_args!("{:x}", report.registers[5]));
        serial::write_str(" rbp=0x");
        serial::write_fmt(core::format_args!("{:x}", report.registers[6]));
        serial::write_str(" rsp=0x");
        serial::write_fmt(core::format_args!("{:x}", report.registers[7]));
        serial::write_newline();
    }
}

/// Report a page fault the kernel detected itself.
///
/// The diagnostic is the same one the unified exception path prints.
pub fn report_faulting_page_fault(fault_address: u64, error_code: u64) -> ! {
    let (rip, rsp) = current_instruction_and_stack_pointer();
    let report = ExceptionReport {
        name: exception_name(crate::cpu::vectors::vector::PAGE_FAULT).unwrap_or("Page Fault"),
        vector: crate::cpu::vectors::vector::PAGE_FAULT,
        error_code,
        has_error_code: true,
        rip,
        rsp,
        cs: 0,
        rflags: 0,
        fault_address: Some(fault_address),
        registers: [0; 8],
    };
    print_report(&report);
    terminate()
}

/// The address of the calling instruction and the current stack pointer.
fn current_instruction_and_stack_pointer() -> (u64, u64) {
    let rip: u64;
    unsafe {
        core::arch::asm!(
            "lea {0}, [rip]",
            out(reg) rip,
            options(nomem, nostack, preserves_flags)
        );
    }
    let rsp: u64;
    unsafe {
        core::arch::asm!("mov {0}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags));
    }
    (rip, rsp)
}

/// Report a vector that has no handler and terminate the machine.
fn unexpected_vector(frame: &InterruptFrame, vector: u8) -> ! {
    unsafe {
        serial::write_str("Elvaris kernel: EXCEPTION: Unexpected Vector\r\n");
        serial::write_str("Elvaris kernel: exception: vector=");
        serial::write_fmt(core::format_args!("{}", vector));
        serial::write_str(" rip=0x");
        serial::write_fmt(core::format_args!("{:x}", frame.rip()));
        serial::write_newline();
    }
    terminate()
}

/// Terminate the machine after a fatal exception.
fn terminate() -> ! {
    unsafe { exit::exit(exit::EXIT_EXCEPTION) };
    // The exit device ends the emulator; if it is not present the machine
    // has to be stopped here.
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

/// The stack pointer of the interrupted code.
///
/// In 64-bit mode the processor always pushes the stack pointer and the
/// stack segment of the interrupted code, so the frame holds the stack
/// pointer of the interrupted code.
pub fn interrupted_stack_pointer(frame: &InterruptFrame) -> u64 {
    frame.rsp
}

/// Read the control register that holds the address of the last page fault.
pub fn read_cr2() -> u64 {
    let value: u64;
    unsafe {
        core::arch::asm!("mov {}, cr2", out(reg) value, options(nomem, nostack));
    }
    value
}

/// The interrupt stack table entry used for `vector`.
///
/// Only the double fault uses a dedicated stack: it is the one exception that
/// can be raised while the kernel stack is already unusable.
pub fn ist_for_vector(vector: u8) -> u8 {
    if vector == crate::cpu::vectors::vector::DOUBLE_FAULT {
        DOUBLE_FAULT_IST as u8
    } else {
        0
    }
}

/// The code segment selector the interrupt stubs run in.
pub fn handler_selector() -> u16 {
    KERNEL_CODE_SELECTOR
}
