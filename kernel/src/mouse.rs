//! PS/2 mouse packet decoder and a single-producer event queue.
//!
//! The interrupt handler is the only producer. The shell loop is the only
//! consumer. Port setup lives behind `enable` and is not built for host tests.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;

const QUEUE_LEN: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub dx: i16,
    pub dy: i16,
    pub buttons: u8,
}

pub struct Decoder {
    phase: u8,
    flags: u8,
    x: u8,
}

impl Decoder {
    pub const fn new() -> Self {
        Self {
            phase: 0,
            flags: 0,
            x: 0,
        }
    }

    /// Feed one byte from port 0x60. `Some` is a complete movement packet.
    pub fn feed(&mut self, byte: u8) -> Option<Event> {
        match self.phase {
            0 => {
                // Bit 3 is always set in the first byte. Anything else is
                // a leftover and would shift the rest of the packet.
                if byte & 0x08 == 0 {
                    return None;
                }
                self.flags = byte;
                self.phase = 1;
                None
            }
            1 => {
                self.x = byte;
                self.phase = 2;
                None
            }
            _ => {
                let flags = self.flags;
                self.phase = 0;
                Some(Event {
                    dx: sign_extend(self.x, flags & 0x10 != 0),
                    dy: -sign_extend(byte, flags & 0x20 != 0),
                    buttons: flags & 0x07,
                })
            }
        }
    }
}

fn sign_extend(value: u8, negative: bool) -> i16 {
    if negative {
        value as i16 - 256
    } else {
        value as i16
    }
}

struct EventQueue<const N: usize> {
    buf: UnsafeCell<[Event; N]>,
    head: AtomicUsize,
    tail: AtomicUsize,
}

unsafe impl<const N: usize> Sync for EventQueue<N> {}

impl<const N: usize> EventQueue<N> {
    const fn new() -> Self {
        Self {
            buf: UnsafeCell::new([Event {
                dx: 0,
                dy: 0,
                buttons: 0,
            }; N]),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn push(&self, event: Event) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let next = (tail + 1) % N;
        if next == self.head.load(Ordering::Acquire) {
            return false;
        }
        unsafe { (*self.buf.get())[tail] = event };
        self.tail.store(next, Ordering::Release);
        true
    }

    fn pop(&self) -> Option<Event> {
        let head = self.head.load(Ordering::Relaxed);
        if head == self.tail.load(Ordering::Acquire) {
            return None;
        }
        let event = unsafe { (*self.buf.get())[head] };
        self.head.store((head + 1) % N, Ordering::Release);
        Some(event)
    }
}

static DECODER: Mutex<Decoder> = Mutex::new(Decoder::new());
static QUEUE: EventQueue<QUEUE_LEN> = EventQueue::new();

pub fn handle_byte(byte: u8) {
    if let Some(event) = DECODER.lock().feed(byte) {
        QUEUE.push(event);
    }
}

pub fn pop() -> Option<Event> {
    QUEUE.pop()
}

/// Enable the auxiliary PS/2 port and turn on movement packets.
/// Interrupts must be off. Returns false if the controller does not ACK.
#[cfg(not(test))]
pub fn enable() -> bool {
    use x86_64::instructions::port::Port;

    const STATUS: u16 = 0x64;
    const DATA: u16 = 0x60;

    fn status() -> u8 {
        unsafe { Port::<u8>::new(STATUS).read() }
    }

    fn wait_write() -> bool {
        for _ in 0..100_000 {
            if status() & 0x02 == 0 {
                return true;
            }
        }
        false
    }

    fn wait_read() -> bool {
        for _ in 0..100_000 {
            if status() & 0x01 != 0 {
                return true;
            }
        }
        false
    }

    fn write_cmd(cmd: u8) -> bool {
        if !wait_write() {
            return false;
        }
        unsafe { Port::<u8>::new(STATUS).write(cmd) };
        true
    }

    fn write_data(byte: u8) -> bool {
        if !wait_write() {
            return false;
        }
        unsafe { Port::<u8>::new(DATA).write(byte) };
        true
    }

    fn read_data() -> Option<u8> {
        if !wait_read() {
            return None;
        }
        Some(unsafe { Port::<u8>::new(DATA).read() })
    }

    if !write_cmd(0xAD) || !write_cmd(0xA7) {
        return false;
    }
    for _ in 0..16 {
        if status() & 0x01 == 0 {
            break;
        }
        unsafe {
            let _ = Port::<u8>::new(DATA).read();
        }
    }

    let enabled = (|| {
        if !write_cmd(0x20) {
            return None;
        }
        let mut config = read_data()?;
        config |= 0x03;
        config &= !0x20;
        if !write_cmd(0x60) || !write_data(config) {
            return None;
        }
        if !write_cmd(0xA8) || !write_cmd(0xAE) {
            return None;
        }
        if !write_cmd(0xD4) || !write_data(0xF4) {
            return None;
        }
        for _ in 0..8 {
            match read_data() {
                Some(0xFA) => return Some(()),
                Some(_) => continue,
                None => return None,
            }
        }
        None
    })()
    .is_some();

    if !enabled {
        let _ = write_cmd(0xA7);
        let _ = write_cmd(0xAE);
    }
    enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_positive_packet() {
        let mut decoder = Decoder::new();
        assert_eq!(decoder.feed(0x08), None);
        assert_eq!(decoder.feed(5), None);
        assert_eq!(
            decoder.feed(3),
            Some(Event {
                dx: 5,
                dy: -3,
                buttons: 0,
            })
        );
    }

    #[test]
    fn decodes_negative_axes_and_buttons() {
        let mut decoder = Decoder::new();
        // Bit 3 set, X sign, Y sign, left and right buttons.
        assert_eq!(decoder.feed(0x08 | 0x10 | 0x20 | 0x03), None);
        assert_eq!(decoder.feed(0xFF), None);
        assert_eq!(
            decoder.feed(0xFE),
            Some(Event {
                dx: -1,
                dy: 2,
                buttons: 0x03,
            })
        );
    }

    #[test]
    fn dropped_first_byte_resyncs() {
        let mut decoder = Decoder::new();
        assert_eq!(decoder.feed(0x00), None);
        assert_eq!(decoder.feed(0x08), None);
        assert_eq!(decoder.feed(1), None);
        assert_eq!(
            decoder.feed(0),
            Some(Event {
                dx: 1,
                dy: 0,
                buttons: 0,
            })
        );
    }
}
