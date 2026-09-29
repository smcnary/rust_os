//! Text console on the Limine framebuffer, mirrored to COM1.

use crate::boot::{self, Framebuffer};
use crate::font::FONT8X8;
use crate::serial;
use core::fmt::{self, Write};
use spin::Mutex;

const FG: (u8, u8, u8) = (0xD8, 0xE2, 0xDC);
const BG: (u8, u8, u8) = (0x14, 0x18, 0x1C);

struct Screen {
    fb: Option<Fb>,
    col: usize,
    row: usize,
    cols: usize,
    rows: usize,
}

struct Fb {
    addr: *mut u8,
    pitch: usize,
    width: usize,
    height: usize,
    red: u8,
    green: u8,
    blue: u8,
}

unsafe impl Send for Screen {}

impl Screen {
    const fn empty() -> Self {
        Self {
            fb: None,
            col: 0,
            row: 0,
            cols: 80,
            rows: 25,
        }
    }

    fn attach(&mut self, info: &Framebuffer) {
        if info.bpp != 32 || info.address.is_null() || info.width == 0 || info.height == 0 {
            return;
        }
        self.cols = (info.width as usize) / 8;
        self.rows = (info.height as usize) / 8;
        if self.cols == 0 || self.rows == 0 {
            return;
        }
        self.fb = Some(Fb {
            addr: info.address,
            pitch: info.pitch as usize,
            width: info.width as usize,
            height: info.height as usize,
            red: info.red_mask_shift,
            green: info.green_mask_shift,
            blue: info.blue_mask_shift,
        });
        self.col = 0;
        self.row = 0;
        self.clear();
    }

    fn clear(&mut self) {
        if let Some(fb) = self.fb.as_ref() {
            let bytes = fb.pitch.saturating_mul(fb.height);
            for offset in 0..bytes {
                unsafe { fb.addr.add(offset).write_volatile(0) };
            }
            // Paint the background so blank cells are not pure black.
            for row in 0..self.rows {
                for col in 0..self.cols {
                    self.draw_cell(col, row, b' ', BG, BG);
                }
            }
        }
        self.col = 0;
        self.row = 0;
    }

    fn write_byte(&mut self, byte: u8) {
        serial::write_byte(byte);
        match byte {
            b'\n' => self.newline(),
            b'\r' => self.col = 0,
            0x08 | 0x7f => {
                if self.col > 0 {
                    self.col -= 1;
                    self.draw_cell(self.col, self.row, b' ', FG, BG);
                }
            }
            byte => {
                self.draw_cell(self.col, self.row, byte, FG, BG);
                self.col += 1;
                if self.col >= self.cols {
                    self.newline();
                }
            }
        }
    }

    fn newline(&mut self) {
        self.col = 0;
        self.row += 1;
        if self.row >= self.rows {
            self.scroll();
            self.row = self.rows.saturating_sub(1);
        }
    }

    fn scroll(&mut self) {
        let Some(fb) = self.fb.as_ref() else {
            return;
        };
        let row_bytes = fb.pitch.saturating_mul(8);
        let total = fb.pitch.saturating_mul(fb.height);
        if row_bytes == 0 || row_bytes >= total {
            return;
        }
        for offset in 0..(total - row_bytes) {
            unsafe {
                let byte = fb.addr.add(offset + row_bytes).read_volatile();
                fb.addr.add(offset).write_volatile(byte);
            }
        }
        for row_offset in (total - row_bytes)..total {
            unsafe { fb.addr.add(row_offset).write_volatile(0) };
        }
    }

    fn draw_cell(&self, col: usize, row: usize, ch: u8, fg: (u8, u8, u8), bg: (u8, u8, u8)) {
        let Some(fb) = self.fb.as_ref() else {
            return;
        };
        let glyph = FONT8X8[ch as usize];
        let origin_x = col * 8;
        let origin_y = row * 8;
        let fg = pack(fg, fb);
        let bg = pack(bg, fb);
        for gy in 0..8 {
            let bits = glyph[gy];
            for gx in 0..8 {
                let color = if bits & (1 << gx) != 0 { fg } else { bg };
                put_pixel(fb, origin_x + gx, origin_y + gy, color);
            }
        }
    }
}

fn pack(rgb: (u8, u8, u8), fb: &Fb) -> u32 {
    ((rgb.0 as u32) << fb.red) | ((rgb.1 as u32) << fb.green) | ((rgb.2 as u32) << fb.blue)
}

fn put_pixel(fb: &Fb, x: usize, y: usize, color: u32) {
    if x >= fb.width || y >= fb.height {
        return;
    }
    let offset = y * fb.pitch + x * 4;
    unsafe {
        fb.addr.add(offset).cast::<u32>().write_volatile(color);
    }
}

impl fmt::Write for Screen {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}

static CONSOLE: Mutex<Screen> = Mutex::new(Screen::empty());

pub fn init() {
    let mut screen = CONSOLE.lock();
    if let Some(fb) = boot::framebuffer() {
        screen.attach(fb);
    }
}

pub fn clear() {
    CONSOLE.lock().clear();
}

pub fn write_fmt(args: fmt::Arguments) {
    use core::fmt::Write;
    let _ = CONSOLE.lock().write_fmt(args);
}

pub fn write_panic(args: fmt::Arguments) {
    if let Some(mut screen) = CONSOLE.try_lock() {
        let _ = screen.write_fmt(args);
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::console::write_fmt(format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! println {
    () => { $crate::print!("\n") };
    ($($arg:tt)*) => { $crate::print!("{}\n", format_args!($($arg)*)) };
}
