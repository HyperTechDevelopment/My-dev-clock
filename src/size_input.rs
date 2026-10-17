use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::Span,
    widgets::{Paragraph, Widget},
};

use crate::themes::Theme;

pub const MIN_SCALE: u16 = 1;
pub const MAX_SCALE: u16 = 12;

/// The inline prompt that lets the scale be typed while the clock is running.
///
/// It stays open until `Enter` commits a valid number or `Esc` drops it, and it
/// swallows keys while open so digits never reach the mode shortcuts.
#[derive(Debug, Default)]
pub struct ScaleInput {
    buffer: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScaleAction {
    /// The key changed nothing the app needs to react to.
    Ignored,
    /// Digits changed: the clock should resize immediately for a live preview.
    Preview,
    /// `Enter` with a value inside the allowed range.
    Committed(u16),
    /// `Esc`, or `Enter` with nothing usable typed.
    Cancelled,
}

impl ScaleInput {
    pub fn open() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn on_key(&mut self, key: KeyCode) -> ScaleAction {
        match key {
            KeyCode::Esc => ScaleAction::Cancelled,
            KeyCode::Enter => match self.parse() {
                Some(scale) => ScaleAction::Committed(scale),
                None => ScaleAction::Cancelled,
            },
            KeyCode::Backspace => {
                self.buffer.pop();
                ScaleAction::Preview
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                // Two digits cover every allowed scale, so a third is ignored:
                // typing "105" stays 10 instead of becoming something else.
                if self.buffer.len() < 2 {
                    self.buffer.push(c);
                    ScaleAction::Preview
                } else {
                    ScaleAction::Ignored
                }
            }
            _ => ScaleAction::Ignored,
        }
    }

    /// The scale the buffer currently spells, if it is inside the allowed range.
    pub fn parse(&self) -> Option<u16> {
        let value: u16 = self.buffer.parse().ok()?;
        (MIN_SCALE..=MAX_SCALE).contains(&value).then_some(value)
    }

    pub fn prompt(&self) -> String {
        format!(
            " Tamanho da fonte ({MIN_SCALE}-{MAX_SCALE}): {}_   Enter aplica · Esc cancela ",
            self.buffer
        )
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: Theme) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let prompt = self.prompt();
        let width = (prompt.len() as u16).min(area.width);
        let line = Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.bottom().saturating_sub(1),
            width,
            height: 1,
        };
        let style = if self.parse().is_some() {
            theme.accent
        } else {
            theme.header
        };
        Paragraph::new(Span::styled(prompt, style)).render(line, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(keys: &str) -> ScaleInput {
        let mut input = ScaleInput::open();
        for c in keys.chars() {
            input.on_key(KeyCode::Char(c));
        }
        input
    }

    #[test]
    fn digits_build_a_scale_and_enter_commits_it() {
        let mut input = typed("7");
        assert_eq!(input.on_key(KeyCode::Enter), ScaleAction::Committed(7));
    }

    #[test]
    fn two_digit_values_are_accepted() {
        let input = typed("12");
        assert_eq!(input.parse(), Some(12));
    }

    #[test]
    fn a_third_digit_is_ignored_once_the_value_is_valid() {
        let input = typed("105");
        assert_eq!(input.parse(), Some(10));
    }

    #[test]
    fn zero_is_out_of_range_so_a_second_digit_still_fits() {
        let mut input = typed("0");
        assert_eq!(input.parse(), None);
        input.on_key(KeyCode::Char('5'));
        assert_eq!(input.parse(), Some(5));
    }

    #[test]
    fn values_above_the_maximum_are_not_committed() {
        // "19" passa do máximo, então não pode ser confirmado com Enter.
        let mut input = typed("19");
        assert_eq!(input.parse(), None);
        assert_eq!(input.on_key(KeyCode::Enter), ScaleAction::Cancelled);
    }

    #[test]
    fn escape_and_empty_enter_cancel() {
        let mut input = typed("3");
        assert_eq!(input.on_key(KeyCode::Esc), ScaleAction::Cancelled);

        let mut input = ScaleInput::open();
        assert_eq!(input.on_key(KeyCode::Enter), ScaleAction::Cancelled);
    }

    #[test]
    fn backspace_clears_the_buffer() {
        let mut input = typed("11");
        input.on_key(KeyCode::Backspace);
        assert_eq!(input.parse(), Some(1));
        input.on_key(KeyCode::Backspace);
        assert_eq!(input.parse(), None);
    }

    #[test]
    fn prompt_shows_the_range_and_the_typed_value() {
        let input = typed("4");
        assert!(input.prompt().contains("(1-12)"));
        assert!(input.prompt().contains(": 4_"));
    }
}
