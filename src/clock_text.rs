use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};

/// Space between two characters of the big clock.
pub const CHARACTER_SPACING: u16 = 2;

/// The digit styles the clock can be drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontStyle {
    /// Solid blocks on a 6x5 grid. The original look.
    Bricks,
    /// One cell wide strokes on a 4x7 grid: taller and lighter.
    Slim,
    /// Bricks leaning to the right, like an italic face.
    Slant,
    /// Each digit inside a filled card split by a hinge line, the flip clock
    /// look: bright cards with dark digits and a cut across the middle.
    Flip,
}

/// The styles registered out of the box, in `Shift+F` cycle order.
pub const FONTS: [FontStyle; 4] = [
    FontStyle::Bricks,
    FontStyle::Slim,
    FontStyle::Slant,
    FontStyle::Flip,
];

impl FontStyle {
    pub fn name(self) -> &'static str {
        match self {
            Self::Bricks => "bricks",
            Self::Slim => "slim",
            Self::Slant => "slant",
            Self::Flip => "flip",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        FONTS.into_iter().find(|style| style.name() == name)
    }

    /// The card face is the one that reads as a flap clock, so it turns the
    /// flip animation on unless the config says otherwise.
    pub fn is_flip(self) -> bool {
        matches!(self, Self::Flip)
    }

    /// Names for help text and error messages.
    pub fn list() -> String {
        FONTS
            .iter()
            .map(|style| style.name())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Cells per glyph at scale 1, before the shear and the card padding.
    pub fn columns(self) -> u16 {
        match self {
            Self::Slim => 4,
            _ => 6,
        }
    }

    /// Rows per glyph at scale 1.
    pub fn rows(self) -> u16 {
        match self {
            Self::Slim => 7,
            _ => 5,
        }
    }

    /// How far the top of the glyph leans right.
    fn shear(self) -> u16 {
        match self {
            Self::Slant => 2,
            _ => 0,
        }
    }

    /// Cells between two characters. The narrow face gets a tighter gap so
    /// its thin strokes still read as one string.
    fn spacing(self) -> u16 {
        match self {
            Self::Slim => 1,
            _ => CHARACTER_SPACING,
        }
    }

    /// Padding inside the card, one cell on each side.
    fn card_padding(self) -> u16 {
        match self {
            Self::Flip => 2,
            _ => 0,
        }
    }
}

/// Glyph rows. A space means nothing is drawn in that cell, and any other
/// character is painted as is, which is what lets one table describe block
/// digits and thin strokes alike.
type Glyph = &'static [&'static str];

fn bricks(c: char) -> Option<Glyph> {
    Some(match c {
        '0' => &["██████", "██  ██", "██  ██", "██  ██", "██████"],
        '1' => &["████  ", "  ██  ", "  ██  ", "  ██  ", "██████"],
        '2' => &["██████", "    ██", "██████", "██    ", "██████"],
        '3' => &["██████", "    ██", "██████", "    ██", "██████"],
        '4' => &["██  ██", "██  ██", "██████", "    ██", "    ██"],
        '5' => &["██████", "██    ", "██████", "    ██", "██████"],
        '6' => &["██████", "██    ", "██████", "██  ██", "██████"],
        '7' => &["██████", "    ██", "    ██", "    ██", "    ██"],
        '8' => &["██████", "██  ██", "██████", "██  ██", "██████"],
        '9' => &["██████", "██  ██", "██████", "    ██", "██████"],
        ':' => &["      ", "  ██  ", "      ", "  ██  ", "      "],
        '.' => &["      ", "      ", "      ", "      ", "  ██  "],
        '-' => &["      ", "      ", "██████", "      ", "      "],
        '°' => &["██  ██", "██  ██", "██    ", "      ", "      "],
        _ => return None,
    })
}

fn slim(c: char) -> Option<Glyph> {
    Some(match c {
        '0' => &[" ██ ", "█  █", "█  █", "█  █", "█  █", "█  █", " ██ "],
        '1' => &["  █ ", " ██ ", "  █ ", "  █ ", "  █ ", "  █ ", " ███"],
        '2' => &[" ██ ", "█  █", "   █", "  █ ", " █  ", "█   ", "████"],
        '3' => &["███ ", "   █", "   █", " ██ ", "   █", "   █", "███ "],
        '4' => &["█  █", "█  █", "█  █", "████", "   █", "   █", "   █"],
        '5' => &["████", "█   ", "█   ", "███ ", "   █", "   █", "███ "],
        '6' => &[" ██ ", "█   ", "█   ", "███ ", "█  █", "█  █", " ██ "],
        '7' => &["████", "   █", "   █", "  █ ", "  █ ", " █  ", " █  "],
        '8' => &[" ██ ", "█  █", "█  █", " ██ ", "█  █", "█  █", " ██ "],
        '9' => &[" ██ ", "█  █", "█  █", " ███", "   █", "   █", " ██ "],
        ':' => &["    ", " ██ ", " ██ ", "    ", " ██ ", " ██ ", "    "],
        '.' => &["    ", "    ", "    ", "    ", "    ", " ██ ", " ██ "],
        '-' => &["    ", "    ", "    ", " ███", "    ", "    ", "    "],
        _ => return None,
    })
}

fn glyph(style: FontStyle, c: char) -> Option<Glyph> {
    match style {
        FontStyle::Slim => slim(c),
        // The flip clock is the block face inside a card.
        FontStyle::Bricks | FontStyle::Slant | FontStyle::Flip => bricks(c),
    }
}

/// A font at a given scale, ready to draw time strings.
pub struct Font {
    style: FontStyle,
    scale: u16,
}

impl Font {
    pub fn new(style: FontStyle, scale: u16) -> Self {
        Self {
            style,
            scale: scale.max(1),
        }
    }

    pub fn style(&self) -> FontStyle {
        self.style
    }

    pub fn scale(&self) -> u16 {
        self.scale
    }

    /// Width of one character, including the shear and the card padding.
    pub fn char_width(&self) -> u16 {
        (self.style.columns() + self.style.shear() + self.style.card_padding()) * self.scale
    }

    pub fn char_height(&self) -> u16 {
        self.style.rows() * self.scale
    }

    /// Distance between the left edge of two neighbouring characters.
    pub fn advance(&self) -> u16 {
        self.char_width().saturating_add(self.style.spacing())
    }

    fn draw_glyph(&self, c: char, x: u16, y: u16, area: Rect, style: Style, buf: &mut Buffer) {
        let Some(rows) = glyph(self.style, c) else {
            return;
        };

        let scale = self.scale;
        let height = rows.len().max(1) as u16;
        for (row_index, row) in rows.iter().enumerate() {
            let row_y = y.saturating_add(row_index as u16 * scale);
            if row_y >= area.bottom() {
                break;
            }
            // The lean eases from the full shear at the top row to zero at
            // the bottom, so the digit reads as tilted instead of snapped.
            let span = height.saturating_sub(1).max(1);
            let lean = ((span.saturating_sub(row_index as u16))
                .saturating_mul(self.style.shear())
                .saturating_mul(scale)
                .saturating_add(span / 2))
                / span;
            let row_x = x.saturating_add(lean);

            for (cell_index, cell) in row.chars().enumerate() {
                if cell == ' ' {
                    continue;
                }
                let cell_x = row_x.saturating_add(cell_index as u16 * scale);
                if cell_x >= area.right() {
                    break;
                }
                let block = cell.to_string().repeat(scale as usize);
                for offset in 0..scale {
                    let line_y = row_y.saturating_add(offset);
                    if line_y >= area.bottom() {
                        break;
                    }
                    let room = area.right().saturating_sub(cell_x) as usize;
                    let block: String = block.chars().take(room).collect();
                    buf.set_string(cell_x, line_y, &block, style);
                }
            }
        }
    }

    /// Draws the card, then the digit cut out of it, then the hinge line across
    /// the middle. Only the flip style uses this.
    fn draw_card(&self, c: char, x: u16, y: u16, area: Rect, style: Style, buf: &mut Buffer) {
        let card = Rect {
            x,
            y,
            width: self.char_width().min(area.right().saturating_sub(x)),
            height: self.char_height().min(area.bottom().saturating_sub(y)),
        };
        if card.width == 0 || card.height == 0 {
            return;
        }

        // The card takes the clock color as its background, so the digits are
        // drawn in the terminal's own background color.
        let color = style.fg.unwrap_or(Color::Green);
        buf.set_style(card, Style::default().bg(color));

        self.draw_glyph(
            c,
            x.saturating_add(self.scale),
            y,
            card,
            Style::default().fg(Color::Black),
            buf,
        );

        // The hinge cuts across the middle but only through the gaps: painted
        // over the digit, it would erase the middle bar of digits like 8.
        let hinge_y = card.y.saturating_add(card.height / 2);
        if hinge_y < card.bottom()
            && let Some(rows) = glyph(self.style, c)
        {
            let mid = rows[rows.len() / 2].chars().collect::<Vec<_>>();
            let origin = x.saturating_add(self.scale);
            for term_x in card.x..card.right() {
                let covered = term_x
                    .checked_sub(origin)
                    .map(|dx| dx / self.scale)
                    .and_then(|cell| mid.get(cell as usize).copied())
                    .is_some_and(|cell| cell != ' ');
                if !covered {
                    buf[(term_x, hinge_y)].set_bg(Color::Black);
                }
            }
        }
    }

    pub fn draw_char(&self, c: char, x: u16, y: u16, area: Rect, style: Style, buf: &mut Buffer) {
        if x >= area.right() || y >= area.bottom() {
            return;
        }

        match self.style {
            FontStyle::Flip => self.draw_card(c, x, y, area, style, buf),
            _ => self.draw_glyph(c, x, y, area, style, buf),
        }
    }

    pub fn draw_str(&self, text: &str, area: Rect, style: Style, buf: &mut Buffer) {
        let advance = self.advance();
        for (index, c) in text.chars().enumerate() {
            let x = area.x.saturating_add(index as u16 * advance);
            if x >= area.right() {
                break;
            }
            self.draw_char(c, x, area.y, area, style, buf);
        }
    }
}

/// The size a rendered string occupies at a given scale.
pub fn text_size(text: &str, style: FontStyle, scale: u16) -> (u16, u16) {
    let font = Font::new(style, scale);
    let chars = text.chars().count() as u16;
    if chars == 0 {
        return (0, font.char_height());
    }

    (
        chars
            .saturating_mul(font.advance())
            .saturating_sub(font.style.spacing()),
        font.char_height(),
    )
}

/// The largest scale that fits `text` inside `area`, never below 1.
pub fn fit_scale(text: &str, area: Rect, style: FontStyle) -> u16 {
    let chars = text.chars().count() as u16;
    if chars == 0 {
        return 1;
    }

    let columns = style.columns() + style.shear() + style.card_padding();
    let spacing = style.spacing().saturating_mul(chars.saturating_sub(1));
    let by_width = area.width.saturating_sub(spacing) / (columns.saturating_mul(chars));
    let by_height = area.height / style.rows();

    by_width.min(by_height).max(1)
}

/// The height the clock needs at a given scale, header and breathing room
/// included.
pub fn clock_height(style: FontStyle, scale: u16, has_header: bool) -> u16 {
    let padding = if has_header { 4 } else { 2 };
    (style.rows() * scale).saturating_add(padding)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(text: &str, style: FontStyle, scale: u16, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        Font::new(style, scale).draw_str(text, area, Style::default(), &mut buf);

        let mut out = String::new();
        for y in 0..height {
            for x in 0..width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn every_style_knows_its_name_both_ways() {
        for style in FONTS {
            assert_eq!(FontStyle::from_name(style.name()), Some(style));
        }
        assert_eq!(FontStyle::from_name(" Bricks "), Some(FontStyle::Bricks));
        assert_eq!(FontStyle::from_name("comic-sans"), None);
        assert!(FontStyle::Flip.is_flip());
        assert!(!FontStyle::Bricks.is_flip());
        assert!(FontStyle::list().contains("flip"));
    }

    #[test]
    fn the_block_digits_keep_their_original_shape() {
        assert_eq!(
            frame("1", FontStyle::Bricks, 1, 6, 5),
            "████  \n  ██  \n  ██  \n  ██  \n██████\n"
        );
        assert_eq!(
            frame("0", FontStyle::Bricks, 1, 6, 5),
            "██████\n██  ██\n██  ██\n██  ██\n██████\n"
        );
    }

    #[test]
    fn the_slim_face_is_four_by_seven() {
        assert_eq!(FontStyle::Slim.columns(), 4);
        assert_eq!(FontStyle::Slim.rows(), 7);
        assert_eq!(
            frame("0", FontStyle::Slim, 1, 4, 7),
            " ██ \n█  █\n█  █\n█  █\n█  █\n█  █\n ██ \n"
        );
    }

    #[test]
    fn the_slant_face_leans_to_the_right() {
        let upright = frame("1", FontStyle::Bricks, 1, 8, 5);
        let leaning = frame("1", FontStyle::Slant, 1, 8, 5);
        assert_ne!(upright, leaning);

        // O topo começa mais à direita que a base: essa inclinação é a
        // diferença inteira em relação à face reta.
        let leftmost = |line: &str| line.find('█').expect("tem bloco");
        let top = leftmost(leaning.lines().next().expect("primeira linha"));
        let bottom = leftmost(leaning.lines().last().expect("última linha"));
        assert!(top > bottom, "{leaning}");
    }

    #[test]
    fn the_slant_lean_eases_row_by_row_instead_of_snapping() {
        // Uma inclinação que pula duas células de uma linha para outra parte
        // o dígito no meio; o degrau máximo é uma célula por linha e escala.
        for scale in 1..=3 {
            let area = Rect::new(0, 0, 40, 20);
            let mut buf = Buffer::empty(area);
            Font::new(FontStyle::Slant, scale).draw_str("8", area, Style::default(), &mut buf);

            let mut previous: Option<usize> = None;
            for y in 0..5 * scale {
                let leftmost = (0..40u16).find(|x| buf[(*x, y)].symbol() == "█");
                let leftmost = leftmost.map(usize::from);
                if let (Some(leftmost), Some(previous)) = (leftmost, previous) {
                    let step = leftmost.abs_diff(previous);
                    assert!(
                        step <= scale as usize,
                        "degrau de {step} na linha {y} com escala {scale}"
                    );
                    assert!(
                        leftmost <= previous,
                        "a inclinação deveria cair da esquerda para a direita de cima para baixo (linha {y}, escala {scale})"
                    );
                }
                previous = leftmost;
            }
        }
    }

    #[test]
    fn the_flip_face_wraps_each_digit_in_a_card() {
        let area = Rect::new(0, 0, 8, 5);
        let mut buf = Buffer::empty(area);
        Font::new(FontStyle::Flip, 1).draw_str(
            "1",
            area,
            Style::default().fg(Color::Green),
            &mut buf,
        );

        // O cartão preenche a caixa inteira com a cor do relógio. A dobradiça
        // aparece só nos vãos do dígito: onde há bloco, o fundo continua o
        // do cartão para a barra do meio não sumir.
        for x in 0..8 {
            for y in 0..5 {
                let digit_here = y == 2 && (3..=4).contains(&x);
                let expected = if y == 2 && !digit_here {
                    Color::Black
                } else {
                    Color::Green
                };
                assert_eq!(buf[(x, y)].bg, expected, "célula {x},{y} do cartão");
            }
        }
    }

    #[test]
    fn the_flip_card_paints_the_digit_dark() {
        let area = Rect::new(0, 0, 8, 5);
        let mut buf = Buffer::empty(area);
        Font::new(FontStyle::Flip, 1).draw_str(
            "8",
            area,
            Style::default().fg(Color::Green),
            &mut buf,
        );

        let dark_digit = (0..5)
            .any(|y| (0..8).any(|x| buf[(x, y)].symbol() == "█" && buf[(x, y)].fg == Color::Black));
        assert!(dark_digit, "o dígito deveria ser escuro sobre o cartão");
    }

    #[test]
    fn the_flip_hinge_keeps_the_middle_bar_visible() {
        let area = Rect::new(0, 0, 8, 5);
        let mut buf = Buffer::empty(area);
        Font::new(FontStyle::Flip, 1).draw_str(
            "8",
            area,
            Style::default().fg(Color::Green),
            &mut buf,
        );

        // A fileira do meio do 8 é cheia: cada célula dela mostra bloco
        // escuro, não um corte preto.
        for x in 1..=6 {
            assert_eq!(buf[(x, 2)].symbol(), "█", "bloco sumiu em {x},2");
            assert_eq!(buf[(x, 2)].fg, Color::Black, "cor errada em {x},2");
        }
    }

    #[test]
    fn the_slim_face_uses_a_tighter_gap() {
        assert_eq!(FontStyle::Slim.spacing(), 1);
        assert_eq!(FontStyle::Bricks.spacing(), CHARACTER_SPACING);
        assert_eq!(text_size("88", FontStyle::Slim, 1).0, 4 * 2 + 1);
    }

    #[test]
    fn sizes_grow_with_the_scale() {
        let small = text_size("88", FontStyle::Bricks, 1);
        let large = text_size("88", FontStyle::Bricks, 3);

        assert!(large.0 > small.0 && large.1 > small.1);
        assert_eq!(small, (6 * 2 + CHARACTER_SPACING, 5));
        assert_eq!(text_size("", FontStyle::Slim, 2), (0, 14));
    }

    #[test]
    fn the_flip_card_makes_the_characters_wider() {
        let bricks = text_size("88", FontStyle::Bricks, 1).0;
        let flip = text_size("88", FontStyle::Flip, 1).0;
        assert!(flip > bricks, "{flip} deveria ser maior que {bricks}");
    }

    #[test]
    fn fit_scale_never_goes_below_one_and_respects_the_tighter_axis() {
        assert_eq!(
            fit_scale("00:00:00", Rect::new(0, 0, 4, 2), FontStyle::Bricks),
            1
        );
        assert_eq!(
            fit_scale("00", Rect::new(0, 0, 200, 10), FontStyle::Bricks),
            2
        );
        assert_eq!(
            fit_scale("00", Rect::new(0, 0, 40, 200), FontStyle::Bricks),
            3
        );
        // The slim face is narrower, so the same width fits a bigger scale.
        assert_eq!(
            fit_scale("00", Rect::new(0, 0, 21, 200), FontStyle::Slim),
            2
        );
    }

    #[test]
    fn drawing_never_writes_outside_the_area() {
        for style in FONTS {
            let area = Rect::new(1, 1, 30, 10);
            let mut buf = Buffer::empty(Rect::new(0, 0, 60, 16));
            Font::new(style, 2).draw_str("12:34", area, Style::default(), &mut buf);

            for y in 0..16 {
                for x in 0..60 {
                    if area.contains((x, y).into()) {
                        continue;
                    }
                    let cell = &buf[(x, y)];
                    assert_eq!(
                        cell.symbol(),
                        " ",
                        "{} escreveu fora da área em {x},{y}",
                        style.name()
                    );
                    assert_eq!(
                        cell.bg,
                        Color::Reset,
                        "{} pintou fundo fora da área em {x},{y}",
                        style.name()
                    );
                }
            }
        }
    }

    #[test]
    fn every_style_draws_every_character_it_supports() {
        for style in FONTS {
            for c in "0123456789:.-".chars() {
                assert!(
                    glyph(style, c).is_some(),
                    "{} sem o glifo {c}",
                    style.name()
                );
            }
        }
        assert!(glyph(FontStyle::Bricks, 'x').is_none());
    }

    #[test]
    fn the_clock_height_grows_with_the_scale_and_the_header() {
        assert_eq!(clock_height(FontStyle::Bricks, 2, false), 12);
        assert_eq!(clock_height(FontStyle::Bricks, 2, true), 14);
        assert_eq!(clock_height(FontStyle::Slim, 2, false), 16);
    }
}
