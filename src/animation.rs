use std::time::Duration;

use ratatui::{buffer::Buffer, layout::Rect, style::Style};

/// The background animations, in `Shift+A` cycle order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Animation {
    /// No background: the terminal shows through.
    None,
    /// Columns of characters falling, head bright and tail fading.
    Matrix,
    /// Interfering sine fields, the classic plasma.
    Plasma,
    /// Concentric ripples spreading from the middle.
    Waves,
    /// A sparse field of stars that blink in place.
    Stars,
}

pub const ANIMATIONS: [Animation; 5] = [
    Animation::None,
    Animation::Matrix,
    Animation::Plasma,
    Animation::Waves,
    Animation::Stars,
];

impl Animation {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Matrix => "matrix",
            Self::Plasma => "plasma",
            Self::Waves => "waves",
            Self::Stars => "stars",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        ANIMATIONS.into_iter().find(|kind| kind.name() == name)
    }

    pub fn is_none(self) -> bool {
        matches!(self, Self::None)
    }

    pub fn list() -> String {
        ANIMATIONS
            .iter()
            .map(|kind| kind.name())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Luminance ramp, dark to light: the character that stands in for each
/// brightness level.
const RAMP: [char; 10] = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

/// What the rain and the occasional bright star are made of.
const SPARKS: [char; 24] = [
    'ﾊ', 'ﾐ', 'ﾋ', 'ｰ', 'ｳ', 'ｼ', 'ﾅ', 'ﾓ', 'ﾆ', 'ｻ', 'ﾜ', 'ﾂ', 'ｵ', 'ﾘ', 'ｱ', 'ﾎ', 'ﾃ', 'ﾏ', 'ｹ',
    'ﾒ', 'ｴ', 'ｶ', 'ｷ', 'ﾑ',
];

const RAIN_TAIL: i64 = 9;
/// Terminal cells are about twice as tall as they are wide, so horizontal
/// frequencies are halved: without that correction every round shape comes out
/// stretched vertically, which is the usual giveaway of a naive ASCII field.
const CELL_ASPECT: f32 = 0.5;

/// Everything the animation needs besides the area it paints.
#[derive(Clone, Copy, Debug)]
pub struct AnimationFrame {
    /// Style for the dimmed body of the animation.
    pub dim: Style,
    /// Style for the highlights: the head of the rain, the crest of a ring.
    pub accent: Style,
    /// How long the app has been running.
    pub elapsed: Duration,
    /// Fixed seed, so the same inputs always give the same frame.
    pub seed: u64,
    /// Frames per second the motion is stepped at.
    pub fps: u32,
}

/// Paints one frame of `kind` over `area`, at a time quantised to `frame.fps`.
///
/// Time is injected and randomness comes from the seed, so the same inputs
/// always produce the same frame: that is what makes the animation testable
/// instead of something only the eye can check.
pub fn render(kind: Animation, area: Rect, buf: &mut Buffer, frame: AnimationFrame) {
    if kind.is_none() || area.width == 0 || area.height == 0 {
        return;
    }

    // Stepping the clock in whole frames is what gives the retro choppy motion,
    // and it keeps the drawing cost bounded on a slow terminal.
    let fps = frame.fps.clamp(1, 60) as f32;
    let tick = (frame.elapsed.as_secs_f32() * fps) as u64;
    let time = tick as f32 / fps;

    match kind {
        Animation::None => {}
        Animation::Matrix => rain(area, buf, frame.dim, frame.accent, tick, frame.seed),
        Animation::Plasma => plasma(area, buf, frame.dim, time),
        Animation::Waves => waves(area, buf, frame.dim, frame.accent, time),
        Animation::Stars => stars(area, buf, frame.dim, frame.accent, time, frame.seed),
    }
}

/// A deterministic mixer: same inputs, same number, on every machine.
fn mix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn noise(a: u64, b: u64, c: u64) -> u64 {
    mix(a ^ mix(b ^ mix(c)))
}

fn ramp_index(value: f32) -> usize {
    let index = (value * (RAMP.len() - 1) as f32).round();
    index.clamp(0.0, (RAMP.len() - 1) as f32) as usize
}

fn put(buf: &mut Buffer, x: u16, y: u16, ch: char, style: Style) {
    if ch == ' ' {
        return;
    }
    buf.set_string(x, y, ch.to_string(), style);
}

fn rain(area: Rect, buf: &mut Buffer, dim: Style, accent: Style, frame: u64, seed: u64) {
    let height = i64::from(area.height);
    let span = height + RAIN_TAIL;
    if span <= 0 {
        return;
    }

    for column in 0..area.width {
        let column_seed = noise(seed, u64::from(column), 0x5eed);
        let speed = 1 + (column_seed % 3) as i64;
        let start = (column_seed >> 8) % span as u64;
        let head = (frame as i64 * speed + start as i64).rem_euclid(span);

        for depth in 0..RAIN_TAIL {
            let row = head - depth;
            if row < 0 || row >= height {
                continue;
            }
            // The tail thins out with distance, so the column reads as a comet
            // rather than a solid bar.
            if depth > 2 && noise(u64::from(column), row as u64, frame / 4).is_multiple_of(4) {
                continue;
            }
            let ch = SPARKS
                [(noise(u64::from(column), row as u64, frame / 3) % SPARKS.len() as u64) as usize];
            let style = if depth == 0 { accent } else { dim };
            put(buf, area.x + column, area.y + row as u16, ch, style);
        }
    }
}

fn plasma(area: Rect, buf: &mut Buffer, dim: Style, time: f32) {
    for row in 0..area.height {
        let y = f32::from(row);
        for column in 0..area.width {
            let x = f32::from(column) * CELL_ASPECT;
            let value = (x * 0.18 + time * 0.9).sin()
                + (y * 0.18 + time * 0.7).sin()
                + ((x + y) * 0.12 + time * 0.5).sin();

            let ch = RAMP[ramp_index((value + 3.0) / 6.0)];
            put(buf, area.x + column, area.y + row, ch, dim);
        }
    }
}

fn waves(area: Rect, buf: &mut Buffer, dim: Style, accent: Style, time: f32) {
    let centre_x = f32::from(area.width) * 0.5;
    let centre_y = f32::from(area.height) * 0.5;

    for row in 0..area.height {
        // Rows weigh twice as much as columns, which keeps the rings circular.
        let dy = (f32::from(row) - centre_y) / CELL_ASPECT;
        for column in 0..area.width {
            let dx = f32::from(column) - centre_x;
            let distance = (dx * dx + dy * dy).sqrt();
            let value = (distance * 0.32 - time * 3.0).sin();

            let ch = RAMP[ramp_index((value + 1.0) * 0.5)];
            let style = if value > 0.85 { accent } else { dim };
            put(buf, area.x + column, area.y + row, ch, style);
        }
    }
}

fn stars(area: Rect, buf: &mut Buffer, dim: Style, accent: Style, time: f32, seed: u64) {
    for row in 0..area.height {
        for column in 0..area.width {
            let cell = noise(seed, u64::from(column), u64::from(row));
            // Roughly three cells in a hundred hold a star.
            if cell % 100 >= 3 {
                continue;
            }

            let twinkle = ((time * 2.0 + (cell % 100) as f32 * 0.31).sin() + 1.0) * 0.5;
            let (ch, style) = if twinkle > 0.9 {
                (SPARKS[(cell % SPARKS.len() as u64) as usize], accent)
            } else {
                (RAMP[2 + (twinkle * 3.0) as usize], dim)
            };

            put(buf, area.x + column, area.y + row, ch, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    fn dim() -> Style {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::DIM)
    }

    fn accent() -> Style {
        Style::default().fg(Color::LightGreen)
    }

    fn test_frame(elapsed_ms: u64, seed: u64) -> AnimationFrame {
        AnimationFrame {
            dim: dim(),
            accent: accent(),
            elapsed: Duration::from_millis(elapsed_ms),
            seed,
            fps: 12,
        }
    }

    fn frame(kind: Animation, elapsed_ms: u64, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        render(kind, area, &mut buf, test_frame(elapsed_ms, 7));

        let mut out = String::new();
        for y in 0..height {
            for x in 0..width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn painted(text: &str) -> usize {
        text.chars().filter(|c| *c != ' ' && *c != '\n').count()
    }

    #[test]
    fn every_animation_knows_its_name_both_ways() {
        for kind in ANIMATIONS {
            assert_eq!(Animation::from_name(kind.name()), Some(kind));
        }
        assert_eq!(Animation::from_name(" PLASMA "), Some(Animation::Plasma));
        assert_eq!(Animation::from_name("nenhuma"), None);
        assert!(Animation::list().contains("stars"));
    }

    #[test]
    fn the_same_time_always_draws_the_same_frame() {
        for kind in ANIMATIONS {
            let first = frame(kind, 1500, 40, 12);
            let second = frame(kind, 1500, 40, 12);
            assert_eq!(first, second, "{} não é determinística", kind.name());
        }
    }

    #[test]
    fn time_moves_so_a_later_frame_differs() {
        for kind in [
            Animation::Matrix,
            Animation::Plasma,
            Animation::Waves,
            Animation::Stars,
        ] {
            let first = frame(kind, 0, 40, 12);
            let later = frame(kind, 900, 40, 12);
            assert_ne!(first, later, "{} ficou parada", kind.name());
        }
    }

    #[test]
    fn time_is_stepped_by_the_frame_rate() {
        // Two moments inside the same frame at 12 fps must draw the same thing.
        assert_eq!(
            frame(Animation::Plasma, 0, 30, 10),
            frame(Animation::Plasma, 20, 30, 10)
        );
        assert_ne!(
            frame(Animation::Plasma, 0, 30, 10),
            frame(Animation::Plasma, 100, 30, 10)
        );
    }

    #[test]
    fn the_none_animation_draws_nothing_at_all() {
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        render(Animation::None, area, &mut buf, test_frame(500, 7));

        for y in 0..12 {
            for x in 0..40 {
                assert_eq!(buf[(x, y)].symbol(), " ", "pintou em {x},{y}");
            }
        }
    }

    #[test]
    fn every_animation_paints_something_and_stays_inside_the_area() {
        for kind in ANIMATIONS {
            if kind.is_none() {
                continue;
            }
            let area = Rect::new(2, 2, 30, 8);
            let mut buf = Buffer::empty(Rect::new(0, 0, 40, 14));
            render(kind, area, &mut buf, test_frame(700, 3));

            let inside = (2..10).flat_map(|y| (2..32).map(move |x| (x, y)));
            let painted_inside = inside
                .filter(|(x, y)| buf[(*x, *y)].symbol() != " ")
                .count();
            assert!(painted_inside > 0, "{} não pintou nada", kind.name());

            for y in 0..14 {
                for x in 0..40 {
                    if area.contains((x, y).into()) {
                        continue;
                    }
                    assert_eq!(
                        buf[(x, y)].symbol(),
                        " ",
                        "{} escreveu fora da área em {x},{y}",
                        kind.name()
                    );
                }
            }
        }
    }

    #[test]
    fn the_rain_runs_down_the_screen_over_time() {
        let first = frame(Animation::Matrix, 0, 30, 12);
        let later = frame(Animation::Matrix, 1200, 30, 12);

        // Every column is a comet, so both frames have plenty of ink, and the
        // column where the head is has moved.
        assert!(painted(&first) > 20, "{first}");
        assert!(painted(&later) > 20, "{later}");

        let head_row = |text: &str, column: usize| -> Option<usize> {
            text.lines()
                .position(|line| line.chars().nth(column).is_some_and(|c| c != ' '))
        };
        assert_ne!(head_row(&first, 5), head_row(&later, 5));
    }

    #[test]
    fn the_plasma_uses_the_whole_brightness_ramp() {
        let drawn = frame(Animation::Plasma, 300, 120, 40);
        let distinct: std::collections::HashSet<char> =
            drawn.chars().filter(|c| *c != '\n').collect();
        assert!(
            distinct.len() >= 5,
            "o plasma deveria usar vários níveis da rampa, usou {distinct:?}"
        );
    }

    #[test]
    fn the_stars_stay_sparse() {
        let drawn = frame(Animation::Stars, 400, 120, 40);
        let total = 120 * 40;
        let density = painted(&drawn) as f32 / total as f32;
        assert!(density < 0.1, "estrelas demais: {density}");
        assert!(density > 0.001, "quase nenhuma estrela: {density}");
    }

    #[test]
    fn the_rings_are_round_not_stretched() {
        // At the same distance from the centre, a cell far to the right and a
        // cell far below should land on the same ramp level. Without the cell
        // aspect correction the vertical ring would be half the size it should.
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        render(Animation::Waves, area, &mut buf, test_frame(0, 5));

        let centre = (30u16, 10u16);
        let level = |x: u16, y: u16| {
            RAMP.iter()
                .position(|c| buf[(x, y)].symbol() == c.to_string())
        };
        // 8 columns to the right and 4 rows down are the same visual distance.
        let right = level(centre.0 + 16, centre.1);
        let below = level(centre.0, centre.1 + 8);
        assert!(right.is_some() && below.is_some());
        assert!(
            right.unwrap().abs_diff(below.unwrap()) <= 1,
            "os anéis estão esticados: direita={right:?} baixo={below:?}"
        );
    }

    #[test]
    fn a_seed_change_redraws_the_random_animations() {
        let area = Rect::new(0, 0, 40, 12);
        let draw = |seed: u64| {
            let mut buf = Buffer::empty(area);
            render(Animation::Stars, area, &mut buf, test_frame(300, seed));
            (0..12)
                .flat_map(|y| (0..40).map(move |x| (x, y)))
                .map(|(x, y)| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        };
        assert_ne!(draw(1), draw(2));
    }
}
