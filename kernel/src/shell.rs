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

pub struct Editor {
    buf: [u8; 160],
    len: usize,
}

impl Editor {
    pub const fn new() -> Self {
        Self {
            buf: [0; 160],
            len: 0,
        }
    }

    /// Push one byte. A newline returns the line that was submitted.
    pub fn push(&mut self, byte: u8) -> Option<&str> {
        match byte {
            b'\r' | b'\n' => {
                let len = self.len;
                self.len = 0;
                Some(core::str::from_utf8(&self.buf[..len]).unwrap_or(""))
            }
            0x08 | 0x7f => {
                self.len = self.len.saturating_sub(1);
                None
            }
            byte if (0x20..0x7f).contains(&byte) => {
                if self.len < self.buf.len() {
                    self.buf[self.len] = byte;
                    self.len += 1;
                }
                None
            }
            _ => None,
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
        assert!(editor.push(b'h').is_none());
        assert!(editor.push(b'x').is_none());
        assert!(editor.push(0x08).is_none());
        assert!(editor.push(b'e').is_none());
        assert!(editor.push(b'l').is_none());
        assert!(editor.push(b'p').is_none());
        assert_eq!(editor.push(b'\n'), Some("help"));
        assert_eq!(parse("help"), Command::Help);
    }
}
