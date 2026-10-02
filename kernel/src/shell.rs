//! Line editor and command parser. Neither touches hardware.

pub const HELP: &str = "\
commands:\n\
  help   list commands\n\
  echo   print the rest of the line\n\
  mem    heap and usable RAM\n\
  ticks  timer interrupts since boot\n\
  clear  clear the screen\n\
  theme  list looks or switch to one\n\
  halt   stop the CPU\n";

#[derive(Debug, PartialEq, Eq)]
pub enum Command<'a> {
    Empty,
    Help,
    Echo(&'a str),
    Mem,
    Ticks,
    Clear,
    Theme(Option<&'a str>),
    Halt,
    Unknown(&'a str),
}

pub fn parse(line: &str) -> Command<'_> {
    let line = line.trim();
    if line.is_empty() {
        return Command::Empty;
    }
    let mut parts = line.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();
    match name {
        "help" => Command::Help,
        "echo" => Command::Echo(rest),
        "mem" => Command::Mem,
        "ticks" => Command::Ticks,
        "clear" => Command::Clear,
        "theme" => {
            if rest.is_empty() {
                Command::Theme(None)
            } else {
                Command::Theme(Some(rest))
            }
        }
        "halt" => Command::Halt,
        other => Command::Unknown(other),
    }
}

/// Arrow keys, shared by the PS/2 decoder and serial CSI sequences.
pub const KEY_LEFT: u8 = 0x80;
pub const KEY_RIGHT: u8 = 0x81;
pub const KEY_UP: u8 = 0x82;
pub const KEY_DOWN: u8 = 0x83;

const LINE_CAP: usize = 160;
const HISTORY_CAP: usize = 16;

#[derive(Debug, PartialEq, Eq)]
pub enum Effect<'a> {
    Ignored,
    Redraw,
    Submitted(&'a str),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Esc {
    None,
    Esc,
    Csi,
}

pub struct Editor {
    buf: [u8; LINE_CAP],
    len: usize,
    cursor: usize,
    history: [[u8; LINE_CAP]; HISTORY_CAP],
    hist_len: [usize; HISTORY_CAP],
    hist_count: usize,
    hist_next: usize,
    /// Steps back from the newest history entry. `None` is the live line.
    view: Option<usize>,
    draft: [u8; LINE_CAP],
    draft_len: usize,
    draft_cursor: usize,
    esc: Esc,
}

impl Editor {
    pub const fn new() -> Self {
        Self {
            buf: [0; LINE_CAP],
            len: 0,
            cursor: 0,
            history: [[0; LINE_CAP]; HISTORY_CAP],
            hist_len: [0; HISTORY_CAP],
            hist_count: 0,
            hist_next: 0,
            view: None,
            draft: [0; LINE_CAP],
            draft_len: 0,
            draft_cursor: 0,
            esc: Esc::None,
        }
    }

    pub fn line(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Push one byte from the keyboard or the serial port.
    pub fn push(&mut self, byte: u8) -> Effect<'_> {
        match self.esc {
            Esc::Esc => {
                self.esc = Esc::None;
                if byte == b'[' {
                    self.esc = Esc::Csi;
                    Effect::Ignored
                } else {
                    self.apply(byte)
                }
            }
            Esc::Csi => {
                self.esc = Esc::None;
                match byte {
                    b'D' => self.apply(KEY_LEFT),
                    b'C' => self.apply(KEY_RIGHT),
                    b'A' => self.apply(KEY_UP),
                    b'B' => self.apply(KEY_DOWN),
                    _ => Effect::Ignored,
                }
            }
            Esc::None if byte == 0x1b => {
                self.esc = Esc::Esc;
                Effect::Ignored
            }
            Esc::None => self.apply(byte),
        }
    }

    fn apply(&mut self, byte: u8) -> Effect<'_> {
        match byte {
            b'\r' | b'\n' => {
                let submitted_len = self.len;
                if submitted_len > 0 {
                    self.store_history();
                }
                self.len = 0;
                self.cursor = 0;
                self.view = None;
                self.esc = Esc::None;
                Effect::Submitted(
                    core::str::from_utf8(&self.buf[..submitted_len]).unwrap_or(""),
                )
            }
            0x08 | 0x7f => self.backspace(),
            KEY_LEFT => {
                if self.cursor == 0 {
                    Effect::Ignored
                } else {
                    self.cursor -= 1;
                    Effect::Redraw
                }
            }
            KEY_RIGHT => {
                if self.cursor >= self.len {
                    Effect::Ignored
                } else {
                    self.cursor += 1;
                    Effect::Redraw
                }
            }
            KEY_UP => self.history_up(),
            KEY_DOWN => self.history_down(),
            byte if (0x20..0x7f).contains(&byte) => self.insert(byte),
            _ => Effect::Ignored,
        }
    }

    fn insert(&mut self, byte: u8) -> Effect<'_> {
        if self.len >= LINE_CAP {
            return Effect::Ignored;
        }
        for index in (self.cursor..self.len).rev() {
            self.buf[index + 1] = self.buf[index];
        }
        self.buf[self.cursor] = byte;
        self.cursor += 1;
        self.len += 1;
        Effect::Redraw
    }

    fn backspace(&mut self) -> Effect<'_> {
        if self.cursor == 0 {
            return Effect::Ignored;
        }
        for index in self.cursor..self.len {
            self.buf[index - 1] = self.buf[index];
        }
        self.cursor -= 1;
        self.len -= 1;
        Effect::Redraw
    }

    fn store_history(&mut self) {
        let slot = self.hist_next;
        self.history[slot][..self.len].copy_from_slice(&self.buf[..self.len]);
        self.hist_len[slot] = self.len;
        self.hist_next = (self.hist_next + 1) % HISTORY_CAP;
        if self.hist_count < HISTORY_CAP {
            self.hist_count += 1;
        }
    }

    fn load_view(&mut self, steps: usize) {
        let slot = (self.hist_next + HISTORY_CAP - steps) % HISTORY_CAP;
        let len = self.hist_len[slot];
        self.buf[..len].copy_from_slice(&self.history[slot][..len]);
        self.len = len;
        self.cursor = len;
    }

    fn save_draft(&mut self) {
        self.draft[..self.len].copy_from_slice(&self.buf[..self.len]);
        self.draft_len = self.len;
        self.draft_cursor = self.cursor.min(self.len);
    }

    fn restore_draft(&mut self) {
        self.buf[..self.draft_len].copy_from_slice(&self.draft[..self.draft_len]);
        self.len = self.draft_len;
        self.cursor = self.draft_cursor.min(self.draft_len);
    }

    fn history_up(&mut self) -> Effect<'_> {
        if self.hist_count == 0 {
            return Effect::Ignored;
        }
        let next = match self.view {
            None => {
                self.save_draft();
                1
            }
            Some(steps) if steps < self.hist_count => steps + 1,
            Some(_) => return Effect::Ignored,
        };
        self.view = Some(next);
        self.load_view(next);
        Effect::Redraw
    }

    fn history_down(&mut self) -> Effect<'_> {
        match self.view {
            None => Effect::Ignored,
            Some(1) => {
                self.view = None;
                self.restore_draft();
                Effect::Redraw
            }
            Some(steps) => {
                let steps = steps - 1;
                self.view = Some(steps);
                self.load_view(steps);
                Effect::Redraw
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_command() {
        assert_eq!(parse(""), Command::Empty);
        assert_eq!(parse("   "), Command::Empty);
        assert_eq!(parse("help"), Command::Help);
        assert_eq!(parse("echo hi there"), Command::Echo("hi there"));
        assert_eq!(parse("echo"), Command::Echo(""));
        assert_eq!(parse("mem"), Command::Mem);
        assert_eq!(parse("ticks"), Command::Ticks);
        assert_eq!(parse("clear"), Command::Clear);
        assert_eq!(parse("theme"), Command::Theme(None));
        assert_eq!(parse("theme phosphor"), Command::Theme(Some("phosphor")));
        assert_eq!(parse("theme nope"), Command::Theme(Some("nope")));
        assert_eq!(parse("halt"), Command::Halt);
        assert_eq!(parse("nope"), Command::Unknown("nope"));
        for name in ["help", "echo", "mem", "ticks", "clear", "theme", "halt"] {
            assert!(HELP.contains(name), "{name} missing from help text");
        }
        assert!(HELP.contains("commands:"));
    }

    #[test]
    fn editor_submits_help_and_honours_backspace() {
        let mut editor = Editor::new();
        assert_eq!(editor.push(b'h'), Effect::Redraw);
        assert_eq!(editor.push(b'x'), Effect::Redraw);
        assert_eq!(editor.push(0x08), Effect::Redraw);
        assert_eq!(editor.push(b'e'), Effect::Redraw);
        assert_eq!(editor.push(b'l'), Effect::Redraw);
        assert_eq!(editor.push(b'p'), Effect::Redraw);
        assert_eq!(editor.push(b'\n'), Effect::Submitted("help"));
        assert_eq!(parse("help"), Command::Help);
    }

    #[test]
    fn inserts_and_deletes_at_the_cursor() {
        let mut editor = Editor::new();
        editor.push(b'h');
        editor.push(b'p');
        assert_eq!(editor.push(KEY_LEFT), Effect::Redraw);
        assert_eq!(editor.push(b'e'), Effect::Redraw);
        assert_eq!(editor.line(), "hep");
        assert_eq!(editor.cursor(), 2);
        assert_eq!(editor.push(KEY_LEFT), Effect::Redraw);
        assert_eq!(editor.push(0x08), Effect::Redraw);
        assert_eq!(editor.line(), "ep");
        assert_eq!(editor.cursor(), 0);
        assert_eq!(editor.push(KEY_LEFT), Effect::Ignored);
        assert_eq!(editor.push(0x08), Effect::Ignored);
    }

    #[test]
    fn history_walks_and_restores_the_draft() {
        let mut editor = Editor::new();
        submit(&mut editor, "echo a");
        submit(&mut editor, "echo b");
        editor.push(b'h');
        editor.push(b'i');
        assert_eq!(editor.push(KEY_UP), Effect::Redraw);
        assert_eq!(editor.line(), "echo b");
        assert_eq!(editor.push(KEY_UP), Effect::Redraw);
        assert_eq!(editor.line(), "echo a");
        assert_eq!(editor.push(KEY_UP), Effect::Ignored);
        assert_eq!(editor.push(KEY_DOWN), Effect::Redraw);
        assert_eq!(editor.line(), "echo b");
        assert_eq!(editor.push(KEY_DOWN), Effect::Redraw);
        assert_eq!(editor.line(), "hi");
        assert_eq!(editor.cursor(), 2);
        editor.push(0x08);
        editor.push(0x08);
        assert_eq!(editor.push(b'\n'), Effect::Submitted(""));
        assert_eq!(editor.push(KEY_UP), Effect::Redraw);
        assert_eq!(editor.line(), "echo b");
    }

    #[test]
    fn serial_arrows_match_key_bytes() {
        let mut editor = Editor::new();
        editor.push(b'h');
        editor.push(b'p');
        assert_eq!(editor.push(0x1b), Effect::Ignored);
        assert_eq!(editor.push(b'a'), Effect::Redraw);
        assert_eq!(editor.line(), "hpa");
        editor.push(0x1b);
        editor.push(b'[');
        assert_eq!(editor.push(b'D'), Effect::Redraw);
        editor.push(b'e');
        assert_eq!(editor.line(), "hpea");
        assert_eq!(editor.cursor(), 3);
    }

    fn submit(editor: &mut Editor, line: &str) {
        for byte in line.bytes() {
            editor.push(byte);
        }
        assert!(matches!(editor.push(b'\n'), Effect::Submitted(got) if got == line));
    }
}
