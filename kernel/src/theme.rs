//! Named framebuffer looks. Each one picks a palette and a glyph scale.

pub type Rgb = (u8, u8, u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub name: &'static str,
    pub fg: Rgb,
    pub bg: Rgb,
    /// Glyph pixels on a side. `1` is the 8px font; `2` draws each pixel as a 2x2 block.
    pub scale: u8,
}

pub const THEMES: [Theme; 5] = [
    Theme {
        name: "dusk",
        fg: (0xD8, 0xE2, 0xDC),
        bg: (0x14, 0x18, 0x1C),
        scale: 1,
    },
    Theme {
        name: "phosphor",
        fg: (0x33, 0xFF, 0x66),
        bg: (0x00, 0x00, 0x00),
        scale: 2,
    },
    Theme {
        name: "amber",
        fg: (0xFF, 0xB0, 0x00),
        bg: (0x1A, 0x10, 0x00),
        scale: 2,
    },
    Theme {
        name: "paper",
        fg: (0x1C, 0x19, 0x15),
        bg: (0xF4, 0xF0, 0xE6),
        scale: 1,
    },
    Theme {
        name: "ice",
        fg: (0xD6, 0xE7, 0xFF),
        bg: (0x0B, 0x1E, 0x36),
        scale: 2,
    },
];

pub const DEFAULT: Theme = THEMES[1];

pub fn iter() -> impl Iterator<Item = &'static Theme> {
    THEMES.iter()
}

pub fn by_name(name: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|theme| theme.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_unknown_and_default() {
        let phosphor = by_name("phosphor").unwrap();
        assert_eq!(phosphor.scale, 2);
        assert_eq!(phosphor.fg, (0x33, 0xFF, 0x66));
        assert!(by_name("nope").is_none());
        assert_eq!(DEFAULT.name, "phosphor");
        assert_eq!(DEFAULT.scale, 2);
        let dusk = by_name("dusk").unwrap();
        assert_eq!(dusk.scale, 1);
        assert_eq!(dusk.fg, (0xD8, 0xE2, 0xDC));
        assert_eq!(dusk.bg, (0x14, 0x18, 0x1C));
    }
}
