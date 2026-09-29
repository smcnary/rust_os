//! COM1, polled. The same bytes written to the framebuffer are written here
//! so a headless QEMU session can see the shell.

use core::fmt;
use x86_64::instructions::port::Port;

const DATA: u16 = 0x3F8;
const INT_ENABLE: u16 = 0x3F8 + 1;
const FIFO: u16 = 0x3F8 + 2;
const LINE_CONTROL: u16 = 0x3F8 + 3;
const MODEM_CONTROL: u16 = 0x3F8 + 4;
const LINE_STATUS: u16 = 0x3F8 + 5;

pub fn init() {
    unsafe {
        Port::<u8>::new(INT_ENABLE).write(0x00);
        Port::<u8>::new(LINE_CONTROL).write(0x80);
        Port::<u8>::new(DATA).write(0x01); // 115200 baud
        Port::<u8>::new(INT_ENABLE).write(0x00);
        Port::<u8>::new(LINE_CONTROL).write(0x03); // 8n1
        Port::<u8>::new(FIFO).write(0xC7);
        Port::<u8>::new(MODEM_CONTROL).write(0x0B);
    }
}

pub fn write_str(text: &str) {
    for byte in text.bytes() {
        write_byte(byte);
    }
}

pub fn write_byte(byte: u8) {
    if byte == b'\n' {
        put(b'\r');
    }
    put(byte);
}

pub fn try_read() -> Option<u8> {
    unsafe {
        let status = Port::<u8>::new(LINE_STATUS).read();
        if status & 0x01 == 0 {
            None
        } else {
            Some(Port::<u8>::new(DATA).read())
        }
    }
}

fn put(byte: u8) {
    unsafe {
        let mut status = Port::<u8>::new(LINE_STATUS);
        while status.read() & 0x20 == 0 {}
        Port::<u8>::new(DATA).write(byte);
    }
}

pub struct Writer;

impl fmt::Write for Writer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        write_str(text);
        Ok(())
    }
}
