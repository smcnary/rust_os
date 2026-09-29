//! Kernel entry. Limine jumps to `kernel_main` on the stack it reserved.

#![no_std]
#![no_main]

extern crate alloc;

use rust_os::shell::{self, Command, Editor};
use rust_os::{console, gdt, interrupts, memory, serial};

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    serial::init();
    if !rust_os::boot::revision_supported() {
        serial::write_str("limine base revision not supported\n");
        rust_os::halt();
    }
    console::init();
    rust_os::println!("rust_os ready");
    memory::init();
    exercise_heap();
    gdt::init();
    interrupts::init();
    rust_os::print!("rust_os> ");

    let mut editor = Editor::new();
    loop {
        while let Some(byte) = serial::try_read() {
            on_byte(&mut editor, byte);
        }
        while let Some(byte) = rust_os::keyboard::pop() {
            on_byte(&mut editor, byte);
        }
        x86_64::instructions::hlt();
    }
}

fn on_byte(editor: &mut Editor, byte: u8) {
    if matches!(byte, b'\r' | b'\n') {
        rust_os::print!("\n");
    } else if matches!(byte, 0x08 | 0x7f) {
        rust_os::print!("\x08 \x08");
    } else if (0x20..0x7f).contains(&byte) {
        rust_os::print!("{}", byte as char);
    }

    let Some(line) = editor.push(byte) else {
        return;
    };
    match shell::parse(line) {
        Command::Empty => {}
        Command::Help => rust_os::print!("{}", shell::HELP),
        Command::Echo(text) => rust_os::println!("{text}"),
        Command::Mem => {
            let stats = memory::stats();
            rust_os::println!(
                "heap {} bytes, {} used, usable ram {} bytes",
                stats.heap_bytes,
                stats.used_bytes,
                stats.usable_ram
            );
        }
        Command::Ticks => rust_os::println!("{} ticks", interrupts::ticks()),
        Command::Clear => {
            console::clear();
            serial::write_str("\u{1b}[2J\u{1b}[H");
        }
        Command::Halt => {
            rust_os::println!("halting");
            x86_64::instructions::interrupts::disable();
            rust_os::halt();
        }
        Command::Unknown(name) => rust_os::println!("unknown command: {name}"),
    }
    rust_os::print!("rust_os> ");
}

fn exercise_heap() {
    let mut values = alloc::vec::Vec::new();
    values.extend_from_slice(b"ok");
    assert_eq!(values.as_slice(), b"ok");
}
