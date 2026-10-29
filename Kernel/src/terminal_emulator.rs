//! Terminal emulator (Phase 18.8).

#![allow(dead_code)]

use crate::surface::Surface;

#[derive(Debug)]
pub struct Font {
    char_width: usize,
    char_height: usize,
}

impl Font {
    pub fn new() -> Self {
        Font {
            char_width: 8,
            char_height: 16,
        }
    }

    pub fn char_width(&self) -> usize { self.char_width }
    pub fn char_height(&self) -> usize { self.char_height }
}

impl Default for Font {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TerminalEmulator {
    surface: Surface,
    font: Font,
    cursor_x: usize,
    cursor_y: usize,
    cols: usize,
    rows: usize,
}

impl TerminalEmulator {
    pub fn new(width: usize, height: usize) -> Result<Self, crate::surface::SurfaceError> {
        let font = Font::new();
        let cols = width / font.char_width();
        let rows = height / font.char_height();
        Ok(TerminalEmulator {
            surface: Surface::new(width, height)?,
            font,
            cursor_x: 0,
            cursor_y: 0,
            cols,
            rows,
        })
    }

    pub fn write_char(&mut self, _c: char) {
        self.cursor_x += 1;
        if self.cursor_x >= self.cols {
            self.cursor_x = 0;
            self.cursor_y += 1;
            if self.cursor_y >= self.rows {
                self.cursor_y = self.rows - 1;
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for c in s.chars() {
            self.write_char(c);
        }
    }

    pub fn newline(&mut self) {
        self.cursor_x = 0;
        self.cursor_y += 1;
        if self.cursor_y >= self.rows {
            self.cursor_y = self.rows - 1;
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        }
    }

    pub fn get_surface(&self) -> &Surface {
        &self.surface
    }

    pub fn get_surface_mut(&mut self) -> &mut Surface {
        &mut self.surface
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_terminal() {
        let term = TerminalEmulator::new(640, 480).unwrap();
        assert_eq!(term.cursor_x, 0);
        assert_eq!(term.cursor_y, 0);
    }

    #[test]
    fn write_char() {
        let mut term = TerminalEmulator::new(640, 480).unwrap();
        term.write_char('A');
        assert_eq!(term.cursor_x, 1);
    }

    #[test]
    fn newline() {
        let mut term = TerminalEmulator::new(640, 480).unwrap();
        term.newline();
        assert_eq!(term.cursor_x, 0);
        assert_eq!(term.cursor_y, 1);
    }

    #[test]
    fn backspace() {
        let mut term = TerminalEmulator::new(640, 480).unwrap();
        term.write_char('A');
        term.write_char('B');
        term.backspace();
        assert_eq!(term.cursor_x, 1);
    }

    #[test]
    fn font_init() {
        let font = Font::new();
        assert_eq!(font.char_width(), 8);
        assert_eq!(font.char_height(), 16);
    }
}
