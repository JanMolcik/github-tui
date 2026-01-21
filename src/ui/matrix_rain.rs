use rand::Rng;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::styles;

const MATRIX_CHARS: &[char] = &[
    // Numbers
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    // Latin uppercase
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
    'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
    // Latin lowercase
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm',
    'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    // Greek uppercase
    'Α', 'Β', 'Γ', 'Δ', 'Ε', 'Ζ', 'Η', 'Θ', 'Ι', 'Κ', 'Λ', 'Μ',
    'Ν', 'Ξ', 'Ο', 'Π', 'Ρ', 'Σ', 'Τ', 'Υ', 'Φ', 'Χ', 'Ψ', 'Ω',
    // Greek lowercase
    'α', 'β', 'γ', 'δ', 'ε', 'ζ', 'η', 'θ', 'ι', 'κ', 'λ', 'μ',
    'ν', 'ξ', 'ο', 'π', 'ρ', 'σ', 'τ', 'υ', 'φ', 'χ', 'ψ', 'ω',
    // Cyrillic uppercase
    'А', 'Б', 'В', 'Г', 'Д', 'Е', 'Ж', 'З', 'И', 'Й', 'К', 'Л', 'М',
    'Н', 'О', 'П', 'Р', 'С', 'Т', 'У', 'Ф', 'Х', 'Ц', 'Ч', 'Ш', 'Щ',
    'Ъ', 'Ы', 'Ь', 'Э', 'Ю', 'Я',
    // Cyrillic lowercase
    'а', 'б', 'в', 'г', 'д', 'е', 'ж', 'з', 'и', 'й', 'к', 'л', 'м',
    'н', 'о', 'п', 'р', 'с', 'т', 'у', 'ф', 'х', 'ц', 'ч', 'ш', 'щ',
    'ъ', 'ы', 'ь', 'э', 'ю', 'я',
    // Extended Latin
    'Æ', 'Ø', 'Å', 'æ', 'ø', 'å', 'ß', 'Ð', 'ð', 'Þ', 'þ',
    // Neutral symbols - dots, dashes, pipes
    '.', '·', ':', ';', ',', '\'', '"', '`',
    '-', '–', '—', '_', '~',
    '|', '/', '\\', '!', '?',
    '(', ')', '[', ']', '{', '}',
    // Common symbols
    '@', '#', '$', '%', '&', '*', '+', '=', '<', '>',
];

#[derive(Clone)]
pub struct MatrixColumn {
    pub chars: Vec<char>,
    pub y_pos: f32,
    pub speed: f32,
    pub length: usize,
}

impl MatrixColumn {
    pub fn new(height: usize) -> Self {
        let mut rng = rand::thread_rng();
        let length = rng.gen_range(4..=15);
        let chars: Vec<char> = (0..length)
            .map(|_| MATRIX_CHARS[rng.gen_range(0..MATRIX_CHARS.len())])
            .collect();

        Self {
            chars,
            y_pos: -(rng.gen_range(0..height) as f32),
            speed: rng.gen_range(0.3..1.5),
            length,
        }
    }

    pub fn tick(&mut self, height: usize) {
        let mut rng = rand::thread_rng();
        self.y_pos += self.speed;

        // Reset when column goes off screen
        if self.y_pos as i32 > height as i32 + self.length as i32 {
            self.y_pos = -(rng.gen_range(0..10) as f32);
            self.speed = rng.gen_range(0.3..1.5);
            self.length = rng.gen_range(4..=15);
            self.chars = (0..self.length)
                .map(|_| MATRIX_CHARS[rng.gen_range(0..MATRIX_CHARS.len())])
                .collect();
        }

        // Randomly change a character
        if rng.gen_bool(0.1) && !self.chars.is_empty() {
            let idx = rng.gen_range(0..self.chars.len());
            self.chars[idx] = MATRIX_CHARS[rng.gen_range(0..MATRIX_CHARS.len())];
        }
    }
}

#[derive(Clone)]
pub struct MatrixRain {
    pub columns: Vec<MatrixColumn>,
    pub width: u16,
    pub height: u16,
}

impl Default for MatrixRain {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

impl MatrixRain {
    pub fn new(width: u16, height: u16) -> Self {
        let columns: Vec<MatrixColumn> = (0..width)
            .map(|_| MatrixColumn::new(height as usize))
            .collect();

        Self {
            columns,
            width,
            height,
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.columns = (0..width)
                .map(|_| MatrixColumn::new(height as usize))
                .collect();
        }
    }

    pub fn tick(&mut self) {
        for col in &mut self.columns {
            col.tick(self.height as usize);
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, loading_text: Option<&str>) {
        // Clear and draw border around the matrix area
        frame.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(styles::BORDER_ACTIVE)
            .style(Style::default().bg(Color::Black));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        // Build lines for the matrix effect inside the border
        let mut lines: Vec<Line> = Vec::with_capacity(inner_area.height as usize);

        for row in 0..inner_area.height {
            let mut spans: Vec<Span> = Vec::with_capacity(inner_area.width as usize);

            for col_idx in 0..inner_area.width as usize {
                if col_idx >= self.columns.len() {
                    spans.push(Span::raw(" "));
                    continue;
                }

                let column = &self.columns[col_idx];
                let y = row as i32;
                let col_y = column.y_pos as i32;
                let col_len = column.length as i32;

                // Calculate distance from the head of the column
                let relative_pos = y - col_y;

                if relative_pos >= 0 && relative_pos < col_len {
                    let char_idx = relative_pos as usize;
                    let ch = column.chars.get(char_idx).copied().unwrap_or(' ');

                    // Head character is brightest (white/light green)
                    // Trailing characters fade from bright green to dark green
                    let style = if relative_pos == 0 {
                        Style::default().fg(Color::White)
                    } else if relative_pos == 1 {
                        Style::default().fg(Color::LightGreen)
                    } else if relative_pos < col_len / 2 {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Rgb(0, 100, 0))
                    };

                    spans.push(Span::styled(ch.to_string(), style));
                } else {
                    spans.push(Span::raw(" "));
                }
            }

            lines.push(Line::from(spans));
        }

        // Render the matrix effect
        let matrix = Paragraph::new(lines).style(Style::default().bg(Color::Black));
        frame.render_widget(matrix, inner_area);

        // Render loading text overlay in center
        if let Some(text) = loading_text {
            let text_width = (text.len() as u16 + 4).min(inner_area.width.saturating_sub(2));
            let text_height = 3;
            let x = inner_area.x + (inner_area.width.saturating_sub(text_width)) / 2;
            let y = inner_area.y + (inner_area.height.saturating_sub(text_height)) / 2;

            let popup_area = Rect::new(x, y, text_width, text_height);

            let loading = Paragraph::new(text)
                .style(Style::default().fg(Color::LightGreen).bg(Color::Black))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Green))
                        .style(Style::default().bg(Color::Black)),
                );

            frame.render_widget(Clear, popup_area);
            frame.render_widget(loading, popup_area);
        }
    }
}
