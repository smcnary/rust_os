//! IDT: breakpoint, double fault, PIT timer, and the keyboard.
//!
//! Stable Rust 1.83 does not allow the `x86-interrupt` ABI, so each gate
//! points at an assembly stub. The stub saves caller-saved registers, aligns
//! the stack, and calls an `extern "C"` handler.

use crate::gdt::DOUBLE_FAULT_IST;
use crate::keyboard;
use crate::pic;
use crate::serial;
use core::arch::global_asm;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Lazy;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptDescriptorTable;
use x86_64::VirtAddr;

static TICKS: AtomicU64 = AtomicU64::new(0);

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    unsafe {
        idt.breakpoint
            .set_handler_addr(VirtAddr::new(breakpoint_stub as usize as u64));
        idt.double_fault
            .set_handler_addr(VirtAddr::new(double_fault_stub as usize as u64))
            .set_stack_index(DOUBLE_FAULT_IST);
        idt[32].set_handler_addr(VirtAddr::new(timer_stub as usize as u64));
        idt[33].set_handler_addr(VirtAddr::new(keyboard_stub as usize as u64));
    }
    idt
});

pub fn init() {
    IDT.load();
    pic::init();
    x86_64::instructions::interrupts::enable();
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

extern "C" {
    fn breakpoint_stub();
    fn double_fault_stub();
    fn timer_stub();
    fn keyboard_stub();
}

macro_rules! irq_stub {
    ($name:literal, $handler:ident) => {
        global_asm!(
            concat!(".global ", $name),
            concat!($name, ":"),
            "push rax",
            "push rcx",
            "push rdx",
            "push rsi",
            "push rdi",
            "push r8",
            "push r9",
            "push r10",
            "push r11",
            "push rbx",
            "mov rbx, rsp",
            "and rsp, -16",
            "sub rsp, 8",
            "cld",
            "call {handler}",
            "mov rsp, rbx",
            "pop rbx",
            "pop r11",
            "pop r10",
            "pop r9",
            "pop r8",
            "pop rdi",
            "pop rsi",
            "pop rdx",
            "pop rcx",
            "pop rax",
            "iretq",
            handler = sym $handler,
        );
    };
}

irq_stub!("breakpoint_stub", breakpoint_rust);
irq_stub!("timer_stub", timer_rust);
irq_stub!("keyboard_stub", keyboard_rust);

global_asm!(
    ".global double_fault_stub",
    "double_fault_stub:",
    "call {handler}",
    "2:",
    "hlt",
    "jmp 2b",
    handler = sym double_fault_rust,
);

extern "C" fn timer_rust() {
    TICKS.fetch_add(1, Ordering::Relaxed);
    pic::end_of_interrupt(0);
}

extern "C" fn keyboard_rust() {
    let scancode = unsafe { Port::<u8>::new(0x60).read() };
    keyboard::handle_scancode(scancode);
    pic::end_of_interrupt(1);
}

extern "C" fn breakpoint_rust() {
    serial::write_str("breakpoint\n");
}

extern "C" fn double_fault_rust() {
    serial::write_str("double fault\n");
}
