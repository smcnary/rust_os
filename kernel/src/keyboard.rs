//! PS/2 scancode set 1 decoder and a single-producer byte queue.
//!
//! The interrupt handler is the only producer. The shell loop is the only
//! consumer. Serial input does not use this queue.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;

const QUEUE_LEN: usize = 128;

pub struct Decoder {
    shift: bool,
    extended: bool,
}

impl Decoder {
    pub const fn new() -> Self {
        Self {
            shift: false,
            extended: false,
        }
    }

    /// Feed one byte from port 0x60. `Some` is a character, newline, or backspace.
    pub fn feed(&mut self, scancode: u8) -> Option<u8> {
        if scancode == 0xE0 {
            self.extended = true;
            return None;
        }
        if self.extended {
            self.extended = false;
            return None;
        }
        let released = scancode & 0x80 != 0;
        let code = (scancode & 0x7F) as usize;
        if code == 0x2A || code == 0x36 {
            self.shift = !released;
            return None;
        }
        if released || code >= PLAIN.len() {
            return None;
        }
        let ch = if self.shift { SHIFTED[code] } else { PLAIN[code] };
        if ch == 0 {
            None
        } else {
            Some(ch)
        }
    }
}

pub struct ByteQueue<const N: usize> {
    buf: UnsafeCell<[u8; N]>,
    head: AtomicUsize,
    tail: AtomicUsize,
}

unsafe impl<const N: usize> Sync for ByteQueue<N> {}

impl<const N: usize> ByteQueue<N> {
    pub const fn new() -> Self {
        Self {
            buf: UnsafeCell::new([0; N]),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    pub fn push(&self, byte: u8) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let next = (tail + 1) % N;
        if next == self.head.load(Ordering::Acquire) {
            return false;
        }
        unsafe { (*self.buf.get())[tail] = byte };
        self.tail.store(next, Ordering::Release);
        true
    }

    pub fn pop(&self) -> Option<u8> {
        let head = self.head.load(Ordering::Relaxed);
        if head == self.tail.load(Ordering::Acquire) {
            return None;
        }
        let byte = unsafe { (*self.buf.get())[head] };
        self.head.store((head + 1) % N, Ordering::Release);
        Some(byte)
    }
}

static DECODER: Mutex<Decoder> = Mutex::new(Decoder::new());
static QUEUE: ByteQueue<QUEUE_LEN> = ByteQueue::new();

pub fn handle_scancode(scancode: u8) {
    if let Some(byte) = DECODER.lock().feed(scancode) {
        QUEUE.push(byte);
    }
}

pub fn pop() -> Option<u8> {
    QUEUE.pop()
}

const PLAIN: [u8; 128] = make_plain();
const SHIFTED: [u8; 128] = make_shifted();

const fn make_plain() -> [u8; 128] {
    let mut map = [0u8; 128];
    map[0x02] = b'1';
    map[0x03] = b'2';
    map[0x04] = b'3';
    map[0x05] = b'4';
    map[0x06] = b'5';
    map[0x07] = b'6';
    map[0x08] = b'7';
    map[0x09] = b'8';
    map[0x0A] = b'9';
    map[0x0B] = b'0';
    map[0x0C] = b'-';
    map[0x0D] = b'=';
    map[0x0E] = 0x08;
    map[0x10] = b'q';
    map[0x11] = b'w';
    map[0x12] = b'e';
    map[0x13] = b'r';
    map[0x14] = b't';
    map[0x15] = b'y';
    map[0x16] = b'u';
    map[0x17] = b'i';
    map[0x18] = b'o';
    map[0x19] = b'p';
    map[0x1A] = b'[';
    map[0x1B] = b']';
    map[0x1C] = b'\n';
    map[0x1E] = b'a';
    map[0x1F] = b's';
    map[0x20] = b'd';
    map[0x21] = b'f';
    map[0x22] = b'g';
    map[0x23] = b'h';
    map[0x24] = b'j';
    map[0x25] = b'k';
    map[0x26] = b'l';
    map[0x27] = b';';
    map[0x28] = b'\'';
    map[0x29] = b'`';
    map[0x2B] = b'\\';
    map[0x2C] = b'z';
    map[0x2D] = b'x';
    map[0x2E] = b'c';
    map[0x2F] = b'v';
    map[0x30] = b'b';
    map[0x31] = b'n';
    map[0x32] = b'm';
    map[0x33] = b',';
    map[0x34] = b'.';
    map[0x35] = b'/';
    map[0x39] = b' ';
    map
}

const fn make_shifted() -> [u8; 128] {
    let mut map = make_plain();
    map[0x02] = b'!';
    map[0x03] = b'@';
    map[0x04] = b'#';
    map[0x05] = b'$';
    map[0x06] = b'%';
    map[0x07] = b'^';
    map[0x08] = b'&';
    map[0x09] = b'*';
    map[0x0A] = b'(';
    map[0x0B] = b')';
    map[0x0C] = b'_';
    map[0x0D] = b'+';
    map[0x10] = b'Q';
    map[0x11] = b'W';
    map[0x12] = b'E';
    map[0x13] = b'R';
    map[0x14] = b'T';
    map[0x15] = b'Y';
    map[0x16] = b'U';
    map[0x17] = b'I';
    map[0x18] = b'O';
    map[0x19] = b'P';
    map[0x1A] = b'{';
    map[0x1B] = b'}';
    map[0x1E] = b'A';
    map[0x1F] = b'S';
    map[0x20] = b'D';
    map[0x21] = b'F';
    map[0x22] = b'G';
    map[0x23] = b'H';
    map[0x24] = b'J';
    map[0x25] = b'K';
    map[0x26] = b'L';
    map[0x27] = b':';
    map[0x28] = b'"';
    map[0x29] = b'~';
    map[0x2B] = b'|';
    map[0x2C] = b'Z';
    map[0x2D] = b'X';
    map[0x2E] = b'C';
    map[0x2F] = b'V';
    map[0x30] = b'B';
    map[0x31] = b'N';
    map[0x32] = b'M';
    map[0x33] = b'<';
    map[0x34] = b'>';
    map[0x35] = b'?';
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_letters_shift_and_editing_keys() {
        let mut decoder = Decoder::new();
        assert_eq!(decoder.feed(0x23), Some(b'h'));
        assert_eq!(decoder.feed(0xA3), None);
        assert_eq!(decoder.feed(0x2A), None);
        assert_eq!(decoder.feed(0x23), Some(b'H'));
        assert_eq!(decoder.feed(0xAA), None);
        assert_eq!(decoder.feed(0x23), Some(b'h'));
        assert_eq!(decoder.feed(0x1C), Some(b'\n'));
        assert_eq!(decoder.feed(0x0E), Some(0x08));
        assert_eq!(decoder.feed(0x39), Some(b' '));
        assert_eq!(decoder.feed(0xE0), None);
        assert_eq!(decoder.feed(0x48), None);
    }

    #[test]
    fn queue_is_fifo_until_full() {
        let queue = ByteQueue::<4>::new();
        assert!(queue.push(1));
        assert!(queue.push(2));
        assert!(queue.push(3));
        assert!(!queue.push(4));
        assert_eq!(queue.pop(), Some(1));
        assert!(queue.push(4));
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), Some(3));
        assert_eq!(queue.pop(), Some(4));
        assert_eq!(queue.pop(), None);
    }
}
