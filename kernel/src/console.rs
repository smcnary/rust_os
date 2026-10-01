//! Text console on the Limine framebuffer, mirrored to COM1.

use crate::boot::{self, Framebuffer};
use crate::font::FONT8X8;
use crate::mouse::Event;
use crate::serial;
use crate::theme::{self, Theme};
use core::fmt::{self, Write};
use spin::Mutex;

const MAX_COLS: usize = 256;
const MAX_ROWS: usize = 128;

struct Screen {
    fb: Option<Fb>,
    theme: Theme,
    cells: [u8; MAX_COLS * MAX_ROWS],
    col: usize,
    row: usize,
    cols: usize,
    rows: usize,
    pointer_x: isize,
    pointer_y: isize,
    pointer_col: usize,
    pointer_row: usize,
    buttons: u8,
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
            theme: theme::DEFAULT,
            cells: [b' '; MAX_COLS * MAX_ROWS],
            col: 0,
            row: 0,
            cols: 80,
            rows: 25,
            pointer_x: 0,
            pointer_y: 0,
            pointer_col: 0,
            pointer_row: 0,
            buttons: 0,
        }
    }

    fn cell(&self) -> usize {
        8 * self.theme.scale as usize
    }

    fn attach(&mut self, info: &Framebuffer) {
        if info.bpp != 32 || info.address.is_null() || info.width == 0 || info.height == 0 {
            return;
        }
        let cell = self.cell();
        let cols = ((info.width as usize) / cell).min(MAX_COLS);
        let rows = ((info.height as usize) / cell).min(MAX_ROWS);
        if cols == 0 || rows == 0 {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        self.pointer_x = (cols * cell / 2) as isize;
        self.pointer_y = (rows * cell / 2) as isize;
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

    fn apply(&mut self, name: &str) -> bool {
        let Some(next) = theme::by_name(name) else {
            return false;
        };
        if let Some(fb) = self.fb.as_ref() {
            let cell = 8 * next.scale as usize;
            let cols = (fb.width / cell).min(MAX_COLS);
            let rows = (fb.height / cell).min(MAX_ROWS);
            if cols == 0 || rows == 0 {
                return false;
            }
            self.cols = cols;
            self.rows = rows;
        }
        self.theme = *next;
        self.clear();
        true
    }

    fn clear(&mut self) {
        let bg = self.theme.bg;
        if let Some(fb) = self.fb.as_ref() {
            fill(fb, 0, fb.height, pack(bg, fb));
        }
        for row in 0..self.rows {
            for col in 0..self.cols {
                self.cells[row * MAX_COLS + col] = b' ';
            }
        }
        self.col = 0;
        self.row = 0;
        self.sync_pointer_cell();
        self.paint(self.pointer_col, self.pointer_row);
    }

    fn nudge(&mut self, event: Event) {
        if self.fb.is_none() || self.cols == 0 || self.rows == 0 {
            return;
        }
        let old_col = self.pointer_col;
        let old_row = self.pointer_row;
        let old_buttons = self.buttons;
        self.pointer_x = self.pointer_x.saturating_add(event.dx as isize);
        self.pointer_y = self.pointer_y.saturating_add(event.dy as isize);
        self.buttons = event.buttons;
        self.sync_pointer_cell();
        if old_col != self.pointer_col || old_row != self.pointer_row {
            self.paint(old_col, old_row);
            self.paint(self.pointer_col, self.pointer_row);
        } else if old_buttons != self.buttons {
            self.paint(self.pointer_col, self.pointer_row);
        }
    }

    fn sync_pointer_cell(&mut self) {
        let cell = self.cell();
        if cell == 0 || self.cols == 0 || self.rows == 0 {
            return;
        }
        let max_x = (self.cols * cell).saturating_sub(1) as isize;
        let max_y = (self.rows * cell).saturating_sub(1) as isize;
        self.pointer_x = self.pointer_x.clamp(0, max_x);
        self.pointer_y = self.pointer_y.clamp(0, max_y);
        self.pointer_col = (self.pointer_x as usize / cell).min(self.cols - 1);
        self.pointer_row = (self.pointer_y as usize / cell).min(self.rows - 1);
    }

    fn write_byte(&mut self, byte: u8) {
        serial::write_byte(byte);
        match byte {
            b'\n' => self.newline(),
            b'\r' => self.col = 0,
            0x08 | 0x7f => {
                if self.col > 0 {
                    self.col -= 1;
                    self.put_char(self.col, self.row, b' ');
                }
            }
            byte => {
                self.put_char(self.col, self.row, byte);
                self.col += 1;
                if self.col >= self.cols {
                    self.newline();
                }
            }
        }
    }

    fn put_char(&mut self, col: usize, row: usize, ch: u8) {
        if col < self.cols && row < self.rows {
            self.cells[row * MAX_COLS + col] = ch;
        }
        self.paint(col, row);
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
        self.paint_plain(self.pointer_col, self.pointer_row);
        let cell = self.cell();
        let bg = self.theme.bg;
        let fb = self.fb.as_ref().map(|fb| Fb {
            addr: fb.addr,
            pitch: fb.pitch,
            width: fb.width,
            height: fb.height,
            red: fb.red,
            green: fb.green,
            blue: fb.blue,
        });
        if let Some(fb) = fb.as_ref() {
            let row_bytes = fb.pitch.saturating_mul(cell);
            let total = fb.pitch.saturating_mul(fb.height);
            if row_bytes != 0 && row_bytes < total {
                for offset in 0..(total - row_bytes) {
                    unsafe {
                        let byte = fb.addr.add(offset + row_bytes).read_volatile();
                        fb.addr.add(offset).write_volatile(byte);
                    }
                }
                let start_y = fb.height.saturating_sub(cell);
                fill(fb, start_y, fb.height, pack(bg, fb));
            }
        }
        self.scroll_grid();
        self.paint(self.pointer_col, self.pointer_row);
    }

    fn scroll_grid(&mut self) {
        if self.rows == 0 {
            return;
        }
        for row in 1..self.rows {
            for col in 0..self.cols {
                self.cells[(row - 1) * MAX_COLS + col] = self.cells[row * MAX_COLS + col];
            }
        }
        let last = self.rows - 1;
        for col in 0..self.cols {
            self.cells[last * MAX_COLS + col] = b' ';
        }
    }

    fn paint(&self, col: usize, row: usize) {
        self.paint_cell(col, row, true);
    }

    fn paint_plain(&self, col: usize, row: usize) {
        self.paint_cell(col, row, false);
    }

    fn paint_cell(&self, col: usize, row: usize, highlight: bool) {
        if col >= self.cols || row >= self.rows {
            return;
        }
        let ch = self.cells[row * MAX_COLS + col];
        let on_pointer = highlight && col == self.pointer_col && row == self.pointer_row;
        let (fg, bg) = if on_pointer && self.buttons != 0 {
            (self.theme.fg, self.theme.fg)
        } else if on_pointer {
            (self.theme.bg, self.theme.fg)
        } else {
            (self.theme.fg, self.theme.bg)
        };
        self.draw_cell(col, row, ch, fg, bg);
    }

    fn draw_cell(&self, col: usize, row: usize, ch: u8, fg: (u8, u8, u8), bg: (u8, u8, u8)) {
        let Some(fb) = self.fb.as_ref() else {
            return;
        };
        let scale = self.theme.scale as usize;
        let glyph = FONT8X8[(ch as usize).min(FONT8X8.len() - 1)];
        let origin_x = col * 8 * scale;
        let origin_y = row * 8 * scale;
        let fg = pack(fg, fb);
        let bg = pack(bg, fb);
        for gy in 0..8 {
            let bits = glyph[gy];
            for gx in 0..8 {
                let color = if bits & (1 << gx) != 0 { fg } else { bg };
                for sy in 0..scale {
                    for sx in 0..scale {
                        put_pixel(
                            fb,
                            origin_x + gx * scale + sx,
                            origin_y + gy * scale + sy,
                            color,
                        );
                    }
                }
            }
        }
    }
}

fn pack(rgb: (u8, u8, u8), fb: &Fb) -> u32 {
    ((rgb.0 as u32) << fb.red) | ((rgb.1 as u32) << fb.green) | ((rgb.2 as u32) << fb.blue)
}

fn fill(fb: &Fb, y0: usize, y1: usize, color: u32) {
    for y in y0..y1 {
        for x in 0..fb.width {
            put_pixel(fb, x, y, color);
        }
    }
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

/// Switch to a named look and clear the framebuffer. Unknown names do nothing.
pub fn apply(name: &str) -> bool {
    CONSOLE.lock().apply(name)
}

pub fn current_name() -> &'static str {
    CONSOLE.lock().theme.name
}

pub fn move_pointer(event: Event) {
    CONSOLE.lock().nudge(event);
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
