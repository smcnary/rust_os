//! Freestanding x86_64 kernel library.
//!
//! Host tests compile the scancode decoder, the shell, and the heap.
//! Hardware modules are only built for the kernel.

#![cfg_attr(not(test), no_std)]

pub mod heap;
pub mod keyboard;
pub mod shell;

#[cfg(not(test))]
pub mod boot;
#[cfg(not(test))]
pub mod console;
#[cfg(not(test))]
mod font;
#[cfg(not(test))]
pub mod gdt;
#[cfg(not(test))]
pub mod interrupts;
#[cfg(not(test))]
pub mod memory;
#[cfg(not(test))]
pub mod pic;
#[cfg(not(test))]
pub mod serial;

#[cfg(not(test))]
extern crate alloc;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    serial::write_str("\nkernel panic: ");
    let mut writer = serial::Writer;
    let _ = core::fmt::Write::write_fmt(&mut writer, format_args!("{info}\n"));
    console::write_panic(format_args!("\nkernel panic: {info}\n"));
    halt();
}

/// Stop the CPU until the next interrupt, then forever if interrupts are off.
#[cfg(not(test))]
pub fn halt() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
