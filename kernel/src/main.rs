//! Kernel entry. Limine jumps to `kernel_main` on the stack it reserved.

#![no_std]
#![no_main]

extern crate alloc;

use rust_os::shell::{self, Command, Editor, Effect};
use rust_os::{console, gdt, interrupts, memory, serial, theme};

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

    let mut editor = Editor::new();
    show_prompt(&editor, true);
    loop {
        while let Some(byte) = serial::try_read() {
            on_byte(&mut editor, byte);
        }
        while let Some(byte) = rust_os::keyboard::pop() {
            on_byte(&mut editor, byte);
        }
        while let Some(event) = rust_os::mouse::pop() {
            console::move_pointer(event);
        }
        x86_64::instructions::hlt();
    }
}

const PROMPT: &str = "rust_os> ";

fn show_prompt(editor: &Editor, fresh: bool) {
    if fresh {
        console::begin_line();
    }
    let mut bytes = [0u8; PROMPT.len() + 160];
    let prompt = PROMPT.as_bytes();
    bytes[..prompt.len()].copy_from_slice(prompt);
    let line = editor.line().as_bytes();
    let end = prompt.len() + line.len();
    bytes[prompt.len()..end].copy_from_slice(line);
    let text = core::str::from_utf8(&bytes[..end]).unwrap_or(PROMPT);
    console::redraw_input(text, prompt.len() + editor.cursor());
}

fn on_byte(editor: &mut Editor, byte: u8) {
    match editor.push(byte) {
        Effect::Ignored => {}
        Effect::Redraw => show_prompt(editor, false),
        Effect::Submitted(line) => {
            console::end_input();
            dispatch(line);
            show_prompt(editor, true);
        }
    }
}

fn dispatch(line: &str) {
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
        Command::Theme(None) => {
            let current = console::current_name();
            for look in theme::iter() {
                let mark = if look.name == current { '*' } else { ' ' };
                let px = 8 * look.scale;
                rust_os::println!("{mark} {name:<9} {px}px", name = look.name);
            }
        }
        Command::Theme(Some(name)) => {
            if console::apply(name) {
                rust_os::println!("{name}");
            } else {
                rust_os::println!("unknown theme: {name}");
            }
        }
        Command::Halt => {
            rust_os::println!("halting");
            x86_64::instructions::interrupts::disable();
            rust_os::halt();
        }
        Command::Unknown(name) => rust_os::println!("unknown command: {name}"),
    }
}

fn exercise_heap() {
    let mut values = alloc::vec::Vec::new();
    values.extend_from_slice(b"ok");
    assert_eq!(values.as_slice(), b"ok");
}
