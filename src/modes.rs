use std::io::Write;
use std::time::{Duration as StdDuration, Instant};

use chrono::{DateTime, Datelike, Duration, Local, NaiveDateTime, Utc};
use chrono_tz::Tz;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};

use crate::clock_text::{Font, FontStyle, fit_scale, text_size};
use crate::themes::Theme;
use crate::winproc;

pub const PAUSED_FOOTER: &str = "PAUSADO (espaço para continuar)";

/// Rings the terminal bell. It is a control character, so it cannot disturb the
/// alternate screen, and it is the one notification every terminal shares.
pub fn ring() {
    let mut out = std::io::stdout();
    let _ = out.write_all(b"\x07");
    let _ = out.flush();
}

/// True when every specifier in the format is one chrono knows, so a typo in
/// the config cannot blow up while drawing.
pub fn valid_strftime(format: &str) -> bool {
    !chrono::format::StrftimeItems::new(format)
        .any(|item| matches!(item, chrono::format::Item::Error))
}

/// The short label of a time zone: the part after the slash, with underscores
/// as spaces, so "America/New_York" reads as "New York".
pub fn zone_label(timezone: Tz) -> String {
    timezone
        .name()
        .rsplit('/')
        .next()
        .unwrap_or("?")
        .replace('_', " ")
}
const FLASH_PERIOD_MS: i64 = 1000;
const FLASH_ON_MS: i64 = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationFormat {
    /// Hours, minutes, seconds and tenths.
    HourMinSecDeci,
    /// Hours, minutes and seconds.
    HourMinSec,
    /// Hours and minutes only, so a duration never reads as a bare number.
    HourMin,
}

impl DurationFormat {
    /// Hiding seconds wins over showing tenths: there is no seconds field to
    /// hang the fraction on.
    pub fn from_display(show_seconds: bool, show_millis: bool) -> Self {
        if !show_seconds {
            Self::HourMin
        } else if show_millis {
            Self::HourMinSecDeci
        } else {
            Self::HourMinSec
        }
    }
}

pub fn format_duration(duration: Duration, format: DurationFormat) -> String {
    let negative = duration < Duration::zero();
    let duration = if negative { -duration } else { duration };

    let millis = duration.num_milliseconds();
    let seconds = millis / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    let mut result = String::new();

    fn append(result: &mut String, value: i64) {
        if result.is_empty() {
            result.push_str(&value.to_string());
        } else {
            result.push_str(&format!("{value:02}"));
        }
    }

    if days > 0 {
        result.push_str(&format!("{days}:"));
    }
    match format {
        DurationFormat::HourMin => {
            append(&mut result, hours % 24);
            result.push_str(&format!(":{:02}", minutes % 60));
        }
        DurationFormat::HourMinSecDeci => {
            if hours > 0 {
                append(&mut result, hours % 24);
                result.push(':');
            }
            append(&mut result, minutes % 60);
            result.push_str(&format!(":{:02}.{}", seconds % 60, (millis % 1000) / 100));
        }
        DurationFormat::HourMinSec => {
            if hours > 0 {
                append(&mut result, hours % 24);
                result.push(':');
            }
            append(&mut result, minutes % 60);
            result.push_str(&format!(":{:02}", seconds % 60));
        }
    }

    if negative {
        result.insert(0, '-');
    }
    result
}

fn should_flash(duration: Duration) -> bool {
    duration.num_milliseconds().abs() % FLASH_PERIOD_MS < FLASH_ON_MS
}

fn elapsed_since(started_at: Instant) -> Duration {
    Duration::from_std(started_at.elapsed()).unwrap_or(Duration::MAX)
}

/// Everything a mode wants on screen for one frame.
///
/// Rendering lives here rather than in each mode, so the digits, the face they
/// are drawn with, the live scale change and the flip animation all go through
/// one code path.
pub struct View {
    pub text: String,
    pub header: Option<String>,
    pub footer: Option<String>,
    /// `Some` when the scale was chosen by the user, `None` to fit the terminal.
    pub scale: Option<u16>,
    pub style: Style,
    /// The face the digits are drawn with.
    pub font: FontStyle,
    /// One entry per character while that character is still flipping.
    pub flips: Vec<Option<Flip>>,
}

/// A digit caught in the middle of a split-flap change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flip {
    /// The character being replaced.
    pub previous: char,
    /// `false` while the top half still shows the old character, which is what
    /// makes the change read as a flap falling instead of a jump.
    pub landed: bool,
}

/// Remembers the last character of each position, so a change can be animated.
#[derive(Debug, Default)]
pub struct FlipState {
    previous: Vec<char>,
    pending: Vec<Option<(char, Instant)>>,
    started: bool,
    duration: StdDuration,
}

impl FlipState {
    pub fn new(duration_ms: u64) -> Self {
        Self {
            previous: Vec::new(),
            pending: Vec::new(),
            started: false,
            duration: StdDuration::from_millis(duration_ms),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !self.duration.is_zero()
    }

    /// Advances the state for `text` and reports what is still in flight.
    pub fn update(&mut self, text: &str, now: Instant) -> Vec<Option<Flip>> {
        let chars: Vec<char> = text.chars().collect();
        self.previous.resize(chars.len(), ' ');
        self.pending.resize(chars.len(), None);

        // The first frame only sets the baseline: nothing flips on start-up.
        if !self.started {
            self.started = true;
            self.previous = chars;
            return vec![None; self.previous.len()];
        }

        let duration = self.duration;
        let mut result = Vec::with_capacity(chars.len());

        for (index, c) in chars.iter().copied().enumerate() {
            if self.previous[index] != c {
                self.pending[index] = Some((self.previous[index], now));
            }

            let mut flip = None;
            if let Some((previous, started_at)) = self.pending[index] {
                let elapsed = now.saturating_duration_since(started_at);
                if duration.is_zero() || elapsed >= duration {
                    self.pending[index] = None;
                } else {
                    // The bottom flap falls first and the top one follows.
                    flip = Some(Flip {
                        previous,
                        landed: elapsed * 2 >= duration,
                    });
                }
            }
            result.push(flip);
        }

        self.previous = chars;
        result
    }
}

pub fn render_view(area: Rect, buf: &mut Buffer, view: &View) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let reserved = u16::from(view.header.is_some()) * 2 + u16::from(view.footer.is_some()) * 2;
    let sizing_area = Rect {
        height: area.height.saturating_sub(reserved),
        ..area
    };

    let scale = view
        .scale
        .unwrap_or_else(|| fit_scale(&view.text, sizing_area, view.font));
    let (width, height) = text_size(&view.text, view.font, scale);

    let mut text_area = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    };

    // Nudge the digits down when the header would otherwise sit on top of them.
    if view.header.is_some()
        && text_area.top() <= area.top() + 1
        && text_area.bottom() < area.bottom()
    {
        text_area.y = text_area.y.saturating_add(1);
    }

    // A block clock needs eight columns per character, so a pane narrower than
    // one whole time string would show half of it. Ordinary text keeps the time
    // readable at any width.
    if area.width < text_size(&view.text, view.font, 1).0 {
        draw_plain(area, buf, view);
        return;
    }

    let font = Font::new(view.font, scale);
    if view.flips.iter().all(Option::is_none) {
        font.draw_str(&view.text, text_area, view.style, buf);
    } else {
        draw_flipping(&font, view, text_area, buf);
    }

    let centered = |text: &str, y: u16, style: Style, buf: &mut Buffer| {
        let width = (text.chars().count() as u16).min(area.width);
        let line = Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y,
            width,
            height: 1,
        };
        buf.set_string(line.x, line.y, text, style);
    };

    if let Some(header) = view.header.as_deref()
        && area.top() + 1 < text_area.top()
    {
        centered(header, text_area.top() - 2, view.style, buf);
    }

    if let Some(footer) = view.footer.as_deref()
        && area.bottom() > text_area.bottom() + 1
    {
        centered(footer, text_area.bottom() + 1, view.style, buf);
    }
}

/// The fallback for a pane too narrow for the block face: the time as ordinary
/// text, centered, with the header above it when there is room.
fn draw_plain(area: Rect, buf: &mut Buffer, view: &View) {
    let row = area.y.saturating_add(area.height / 2);

    let line = |text: &str, y: u16, buf: &mut Buffer| {
        let text = truncate_to(text, area.width);
        let width = text.chars().count() as u16;
        buf.set_string(
            area.x.saturating_add(area.width.saturating_sub(width) / 2),
            y,
            &text,
            view.style,
        );
    };

    line(&view.text, row, buf);

    if let Some(header) = view.header.as_deref()
        && row > area.y
    {
        line(header, row - 1, buf);
    }
}

/// Cuts a string to fit a width, without splitting a character.
pub fn truncate_to(text: &str, width: u16) -> String {
    text.chars().take(width as usize).collect()
}

/// Draws the digits one at a time, splitting a digit that is still flipping
/// between its top and bottom halves.
fn draw_flipping(font: &Font, view: &View, text_area: Rect, buf: &mut Buffer) {
    let half = text_area.height / 2;
    let top = Rect {
        height: half,
        ..text_area
    };
    let bottom = Rect {
        y: text_area.y.saturating_add(half),
        height: text_area.height.saturating_sub(half),
        ..text_area
    };

    for (index, c) in view.text.chars().enumerate() {
        let x = text_area.x.saturating_add(index as u16 * font.advance());
        if x >= text_area.right() {
            break;
        }

        match view.flips.get(index).copied().flatten() {
            Some(flip) => {
                // The top half lags behind, still showing the character being
                // replaced until the flap lands.
                let top_char = if flip.landed { c } else { flip.previous };
                font.draw_char(top_char, x, text_area.y, top, view.style, buf);
                font.draw_char(c, x, text_area.y, bottom, view.style, buf);
            }
            None => font.draw_char(c, x, text_area.y, text_area, view.style, buf),
        }
    }
}

pub trait Pause {
    fn is_paused(&self) -> bool;
    fn pause(&mut self);
    fn resume(&mut self);

    fn toggle_paused(&mut self) {
        if self.is_paused() {
            self.resume();
        } else {
            self.pause();
        }
    }
}

/// Localized header date, since chrono formats in English only.
pub fn format_date(date: NaiveDateTime, timezone: Option<Tz>) -> String {
    const WEEKDAYS: [&str; 7] = [
        "domingo",
        "segunda-feira",
        "terça-feira",
        "quarta-feira",
        "quinta-feira",
        "sexta-feira",
        "sábado",
    ];
    const MONTHS: [&str; 12] = [
        "janeiro",
        "fevereiro",
        "março",
        "abril",
        "maio",
        "junho",
        "julho",
        "agosto",
        "setembro",
        "outubro",
        "novembro",
        "dezembro",
    ];

    let weekday = WEEKDAYS[date.weekday().num_days_from_sunday() as usize];
    let month = MONTHS[date.month0() as usize];
    let mut text = format!("{weekday}, {} de {month} de {}", date.day(), date.year());
    if let Some(timezone) = timezone {
        text.push_str(" · ");
        text.push_str(timezone.name());
    }
    text
}

/// Everything the clock needs to know about what it shows.
pub struct ClockOptions {
    pub title: Option<String>,
    pub show_date: bool,
    pub show_seconds: bool,
    pub show_millis: bool,
    pub timezone: Option<Tz>,
    /// strftime for the time. Absent keeps what the display options imply.
    pub format: Option<String>,
    /// strftime for the date line. Absent keeps the localized form.
    pub date_format: Option<String>,
    /// Extra time zones shown next to the date.
    pub world: Vec<Tz>,
}

pub struct Clock {
    pub title: Option<String>,
    pub show_date: bool,
    pub show_seconds: bool,
    pub show_millis: bool,
    pub timezone: Option<Tz>,
    format: Option<String>,
    date_format: Option<String>,
    world: Vec<Tz>,
}

impl Clock {
    pub fn new(options: ClockOptions) -> Self {
        Self {
            title: options.title,
            show_date: options.show_date,
            show_seconds: options.show_seconds,
            show_millis: options.show_millis,
            timezone: options.timezone,
            format: options.format,
            date_format: options.date_format,
            world: options.world,
        }
    }

    fn now(&self) -> NaiveDateTime {
        match self.timezone {
            Some(timezone) => Utc::now().with_timezone(&timezone).naive_local(),
            None => Local::now().naive_local(),
        }
    }

    /// The header line: the title, the date and any extra time zones, joined
    /// with dots.
    pub fn header(&self) -> Option<String> {
        let title = self
            .title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_string);
        let date = self.show_date.then(|| self.date_text());

        let parts: Vec<String> = [title, date, self.world_text()]
            .into_iter()
            .flatten()
            .collect();

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" · "))
        }
    }

    /// The date line, either with the configured format or the localized form.
    fn date_text(&self) -> String {
        let now = self.now();
        match self.date_format.as_deref() {
            Some(format) => now.format(format).to_string(),
            None => format_date(now, self.timezone),
        }
    }

    /// The extra time zones, as "09:12 New York".
    fn world_text(&self) -> Option<String> {
        if self.world.is_empty() {
            return None;
        }

        let now = Utc::now();
        let parts: Vec<String> = self
            .world
            .iter()
            .map(|zone| {
                format!(
                    "{} {}",
                    now.with_timezone(zone).format("%H:%M"),
                    zone_label(*zone)
                )
            })
            .collect();
        Some(parts.join(" · "))
    }

    /// The time string as it is right now, without touching the flip state.
    pub fn text(&self) -> String {
        let now = self.now();
        if let Some(format) = self.format.as_deref() {
            return now.format(format).to_string();
        }
        if !self.show_seconds {
            now.format("%H:%M").to_string()
        } else if self.show_millis {
            now.format("%H:%M:%S%.1f").to_string()
        } else {
            now.format("%H:%M:%S").to_string()
        }
    }

    pub fn view(&self, scale: Option<u16>, font: FontStyle, theme: Theme) -> View {
        // O estado do flip pertence ao app, que lembra dos dígitos anteriores.
        View {
            text: self.text(),
            header: self.header(),
            footer: None,
            scale,
            style: theme.clock,
            font,
            flips: Vec::new(),
        }
    }
}

pub struct Stopwatch {
    pub title: Option<String>,
    pub format: DurationFormat,
    duration: Duration,
    started_at: Option<Instant>,
    laps: Vec<Duration>,
}

impl Stopwatch {
    pub fn new(title: Option<String>, format: DurationFormat, paused: bool) -> Self {
        Self {
            title,
            format,
            duration: Duration::zero(),
            started_at: (!paused).then(Instant::now),
            laps: Vec::new(),
        }
    }

    /// Marks a lap at the current total time.
    pub fn lap(&mut self) {
        self.laps.push(self.total_time());
    }

    pub fn laps(&self) -> &[Duration] {
        &self.laps
    }

    /// The number of the lap in progress and how long it has been running.
    fn current_lap(&self) -> Option<(usize, Duration)> {
        let last = *self.laps.last()?;
        Some((self.laps.len(), self.total_time() - last))
    }

    /// One CSV line per lap: lap number, lap time and total time, in seconds
    /// with millisecond precision. Written when the app exits.
    pub fn laps_csv(&self) -> String {
        let mut out = String::from("volta,volta_s,total_s\n");
        let mut previous = Duration::zero();

        for (index, total) in self.laps.iter().enumerate() {
            let lap = *total - previous;
            out.push_str(&format!(
                "{},{:.3},{:.3}\n",
                index + 1,
                lap.num_milliseconds() as f64 / 1000.0,
                total.num_milliseconds() as f64 / 1000.0
            ));
            previous = *total;
        }

        out
    }

    pub fn total_time(&self) -> Duration {
        match self.started_at {
            Some(started_at) => self.duration + elapsed_since(started_at),
            None => self.duration,
        }
    }

    /// Printed after leaving the alternate screen, so the elapsed time is not
    /// lost when the app exits.
    pub fn summary(&self) -> String {
        format_duration(self.total_time(), DurationFormat::HourMinSecDeci)
    }

    pub fn view(&self, scale: Option<u16>, font: FontStyle, theme: Theme) -> View {
        // The footer shows the lap in progress, so a split is visible while it
        // is being taken instead of only at the end.
        let footer = if self.is_paused() {
            Some(PAUSED_FOOTER.to_string())
        } else {
            self.current_lap().map(|(number, lap)| {
                format!("volta {number} · {}", format_duration(lap, self.format))
            })
        };

        View {
            text: format_duration(self.total_time(), self.format),
            header: self.title.clone(),
            footer,
            scale,
            style: theme.clock,
            font,
            flips: Vec::new(),
        }
    }
}

impl Pause for Stopwatch {
    fn is_paused(&self) -> bool {
        self.started_at.is_none()
    }

    fn pause(&mut self) {
        if let Some(started_at) = self.started_at {
            self.duration += elapsed_since(started_at);
            self.started_at = None;
        }
    }

    fn resume(&mut self) {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
    }
}

/// Everything the timer needs to know about how it should run.
pub struct TimerOptions {
    pub durations: Vec<Duration>,
    pub titles: Vec<String>,
    pub repeat: bool,
    pub format: DurationFormat,
    pub paused: bool,
    pub auto_quit: bool,
    pub execute: Vec<String>,
    /// Ring the terminal bell when the time is up.
    pub bell: bool,
}

pub struct Timer {
    pub durations: Vec<Duration>,
    pub titles: Vec<String>,
    pub repeat: bool,
    pub auto_quit: bool,
    pub execute: Vec<String>,
    pub bell: bool,
    pub format: DurationFormat,
    passed: Duration,
    started_at: Option<Instant>,
    result: Option<String>,
    firing: bool,
}

impl Timer {
    pub fn new(options: TimerOptions) -> Self {
        let durations = if options.durations.is_empty() {
            vec![Duration::minutes(5)]
        } else {
            options.durations
        };

        Self {
            durations,
            titles: options.titles,
            repeat: options.repeat,
            auto_quit: options.auto_quit,
            execute: options.execute,
            bell: options.bell,
            format: options.format,
            passed: Duration::zero(),
            started_at: (!options.paused).then(Instant::now),
            result: None,
            firing: false,
        }
    }

    /// Jumps straight to the end of the interval that is running.
    pub fn skip(&mut self) {
        let (remaining, _) = self.remaining();
        if remaining <= Duration::zero() {
            return;
        }

        let total = self.total_passed();
        self.passed = total + remaining;
        // The interval clock restarts from the new checkpoint.
        if self.started_at.is_some() {
            self.started_at = Some(Instant::now());
        }
    }

    /// Adds or removes whole minutes from the interval that is running.
    pub fn adjust_minutes(&mut self, minutes: i64) {
        let (_, index) = self.remaining();
        let adjusted = self.durations[index] + Duration::minutes(minutes);
        // An interval never gets smaller than a second.
        self.durations[index] = adjusted.max(Duration::seconds(1));
    }

    /// Back to the first interval, keeping the running or paused state.
    pub fn restart(&mut self) {
        self.passed = Duration::zero();
        self.result = None;
        self.firing = false;
        if self.started_at.is_some() {
            self.started_at = Some(Instant::now());
        }
    }

    fn total_passed(&self) -> Duration {
        match self.started_at {
            Some(started_at) => self.passed + elapsed_since(started_at),
            None => self.passed,
        }
    }

    /// Time left in the current interval and which interval that is.
    pub fn remaining(&self) -> (Duration, usize) {
        let passed = self.total_passed();
        let mut index = 0;
        let mut checkpoint = self.durations[0];

        while checkpoint < passed {
            if index >= self.durations.len() - 1 && !self.repeat {
                break;
            }
            index = (index + 1) % self.durations.len();
            checkpoint += self.durations[index];
        }

        (checkpoint - passed, index)
    }

    fn current_title(&self, index: usize) -> Option<String> {
        self.titles
            .get(index.min(self.titles.len().saturating_sub(1)))
            .cloned()
    }

    /// Runs the configured command once when the timer crosses zero.
    pub fn tick(&mut self, theme_name: &str) {
        let (remaining, _) = self.remaining();
        if remaining >= Duration::zero() || self.firing {
            return;
        }

        self.firing = true;
        if self.bell {
            ring();
        }
        self.result = Some(if self.execute.is_empty() {
            String::new()
        } else {
            winproc::run_capture(
                &self.execute,
                winproc::DEFAULT_TIMEOUT,
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                theme_name,
            )
        });
    }

    pub fn is_finished(&self) -> bool {
        self.auto_quit && self.firing
    }

    pub fn view(&self, scale: Option<u16>, font: FontStyle, theme: Theme) -> View {
        let (remaining, index) = self.remaining();
        let over = remaining < Duration::zero();
        let text = format_duration(if over { -remaining } else { remaining }, self.format);

        View {
            text,
            header: self.current_title(index),
            footer: if self.is_paused() {
                Some(PAUSED_FOOTER.to_string())
            } else {
                self.result.clone()
            },
            scale,
            style: theme.clock,
            font,
            flips: Vec::new(),
        }
    }

    /// Fills the area with the accent color while the timer flashes after time
    /// is up, so the screen blinks behind the digits.
    pub fn flash(&self, area: Rect, buf: &mut Buffer, theme: Theme) {
        let (remaining, _) = self.remaining();
        if remaining >= Duration::zero() || !should_flash(remaining) {
            return;
        }

        let background = Style::default().bg(theme.accent.fg.unwrap_or(Color::Green));
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_style(background);
            }
        }
    }
}

impl Pause for Timer {
    fn is_paused(&self) -> bool {
        self.started_at.is_none()
    }

    fn pause(&mut self) {
        if let Some(started_at) = self.started_at {
            self.passed += elapsed_since(started_at);
            self.started_at = None;
        }
    }

    fn resume(&mut self) {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
    }
}

pub struct Countdown {
    pub time: DateTime<Local>,
    pub title: Option<String>,
    pub continue_on_zero: bool,
    pub reverse: bool,
    pub format: DurationFormat,
}

impl Countdown {
    /// True once the target time has passed. A reverse count up never reaches
    /// zero, so it never fires.
    pub fn reached(&self) -> bool {
        !self.reverse && self.remaining() <= Duration::zero()
    }

    fn remaining(&self) -> Duration {
        let remaining = self.time.signed_duration_since(Local::now());
        if self.reverse { -remaining } else { remaining }
    }

    pub fn view(&self, scale: Option<u16>, font: FontStyle, theme: Theme) -> View {
        let remaining = self.remaining();
        let finished = remaining < Duration::zero() && !self.continue_on_zero;
        let text = if finished {
            format_duration(Duration::zero(), self.format)
        } else {
            format_duration(remaining, self.format)
        };

        View {
            text,
            header: self.title.clone(),
            footer: None,
            scale,
            style: theme.clock,
            font,
            flips: Vec::new(),
        }
    }

    /// Countdowns that stop at zero hide the digits for half of each second.
    pub fn hidden(&self) -> bool {
        let remaining = self.remaining();
        remaining < Duration::zero() && !self.continue_on_zero && !should_flash(remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_duration_covers_tenths_seconds_and_minutes() {
        assert_eq!(
            format_duration(
                Duration::milliseconds(65_432),
                DurationFormat::HourMinSecDeci
            ),
            "1:05.4"
        );
        assert_eq!(
            format_duration(Duration::seconds(65), DurationFormat::HourMinSec),
            "1:05"
        );
        assert_eq!(
            format_duration(Duration::seconds(65), DurationFormat::HourMin),
            "0:01"
        );
    }

    #[test]
    fn format_duration_handles_days_and_negative_values() {
        let duration = Duration::days(1) + Duration::hours(2) + Duration::minutes(3);
        assert_eq!(
            format_duration(duration, DurationFormat::HourMinSec),
            "1:02:03:00"
        );
        assert_eq!(
            format_duration(-Duration::seconds(65), DurationFormat::HourMinSecDeci),
            "-1:05.0"
        );
    }

    #[test]
    fn hiding_seconds_takes_priority_over_tenths() {
        assert_eq!(
            DurationFormat::from_display(false, true),
            DurationFormat::HourMin
        );
        assert_eq!(
            DurationFormat::from_display(true, true),
            DurationFormat::HourMinSecDeci
        );
        assert_eq!(
            DurationFormat::from_display(true, false),
            DurationFormat::HourMinSec
        );
    }

    #[test]
    fn a_stopwatch_can_start_paused_and_resume() {
        let mut stopwatch = Stopwatch::new(None, DurationFormat::HourMinSec, true);
        assert!(stopwatch.is_paused());
        assert_eq!(stopwatch.total_time(), Duration::zero());

        stopwatch.resume();
        assert!(!stopwatch.is_paused());
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(stopwatch.total_time() > Duration::zero());

        stopwatch.pause();
        let frozen = stopwatch.total_time();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(stopwatch.total_time(), frozen);
    }

    #[test]
    fn a_timer_without_durations_falls_back_to_five_minutes() {
        let timer = Timer::new(TimerOptions {
            durations: Vec::new(),
            titles: Vec::new(),
            repeat: false,
            format: DurationFormat::HourMinSec,
            paused: true,
            auto_quit: false,
            execute: Vec::new(),
            bell: false,
        });
        assert_eq!(timer.durations, vec![Duration::minutes(5)]);
        assert_eq!(timer.remaining(), (Duration::minutes(5), 0));
    }

    #[test]
    fn a_timer_walks_through_its_intervals_in_order() {
        let mut timer = Timer::new(TimerOptions {
            durations: vec![Duration::minutes(25), Duration::minutes(5)],
            titles: vec!["Foco".to_string(), "Pausa".to_string()],
            repeat: false,
            format: DurationFormat::HourMinSec,
            paused: false,
            auto_quit: false,
            execute: Vec::new(),
            bell: false,
        });

        // Push the clock past the first interval without waiting for it.
        timer.passed = Duration::minutes(26);
        timer.started_at = None;
        let (remaining, index) = timer.remaining();
        assert_eq!(index, 1);
        assert_eq!(remaining, Duration::minutes(4));
        assert_eq!(timer.current_title(index).as_deref(), Some("Pausa"));
    }

    fn clock_with(title: Option<&str>, show_date: bool) -> Clock {
        Clock::new(ClockOptions {
            title: title.map(str::to_string),
            show_date,
            show_seconds: true,
            show_millis: false,
            timezone: None,
            format: None,
            date_format: None,
            world: Vec::new(),
        })
    }

    #[test]
    fn the_header_joins_title_and_date_and_skips_blank_titles() {
        assert_eq!(
            clock_with(Some("Estudo"), false).header().as_deref(),
            Some("Estudo")
        );
        assert_eq!(clock_with(Some("   "), false).header(), None);

        let header = clock_with(Some("Estudo"), true)
            .header()
            .expect("tem cabeçalho");
        assert!(header.starts_with("Estudo · "), "{header}");
    }

    #[test]
    fn the_clock_text_follows_the_display_options() {
        assert_eq!(clock_with(None, false).text().chars().count(), 8);

        let clock = Clock::new(ClockOptions {
            title: None,
            show_date: false,
            show_seconds: false,
            show_millis: false,
            timezone: None,
            format: None,
            date_format: None,
            world: Vec::new(),
        });
        assert_eq!(clock.text().chars().count(), 5);
    }

    #[test]
    fn flip_stays_quiet_until_a_digit_actually_changes() {
        let mut state = FlipState::new(200);
        let start = Instant::now();

        let first = state.update("12:00:00", start);
        assert!(
            first.iter().all(Option::is_none),
            "nada deveria piscar na primeira leitura"
        );

        let same = state.update("12:00:00", start + StdDuration::from_millis(100));
        assert!(same.iter().all(Option::is_none), "sem mudança, sem flip");
    }

    #[test]
    fn a_changed_digit_flips_bottom_first() {
        let mut state = FlipState::new(200);
        let start = Instant::now();
        state.update("12:00:00", start);

        let changed = state.update("12:00:01", start + StdDuration::from_millis(10));
        let flip = changed[7].expect("o último dígito deveria estar virando");
        assert_eq!(flip.previous, '0');
        assert!(!flip.landed, "a metade de baixo cai primeiro");
        assert!(changed[0].is_none(), "os outros dígitos não mudaram");

        let half = state.update("12:00:01", start + StdDuration::from_millis(120));
        assert!(
            half[7].expect("ainda virando").landed,
            "a metade de cima caiu"
        );

        let done = state.update("12:00:01", start + StdDuration::from_millis(260));
        assert!(done[7].is_none(), "o flip deveria ter terminado");
    }

    #[test]
    fn a_zero_duration_turns_the_flip_off() {
        let mut state = FlipState::new(0);
        assert!(!state.is_enabled());

        let start = Instant::now();
        state.update("12:00:00", start);
        let changed = state.update("12:00:01", start + StdDuration::from_millis(5));
        assert!(changed.iter().all(Option::is_none));
    }

    #[test]
    fn the_date_is_localized_with_the_timezone_appended() {
        let date =
            NaiveDateTime::parse_from_str("2026-06-07 01:41:24", "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(format_date(date, None), "domingo, 7 de junho de 2026");
        assert_eq!(
            format_date(date, Some(chrono_tz::America::Sao_Paulo)),
            "domingo, 7 de junho de 2026 · America/Sao_Paulo"
        );
    }

    #[test]
    fn a_reversing_countdown_counts_up() {
        let countdown = Countdown {
            time: Local::now() - Duration::seconds(5),
            title: None,
            continue_on_zero: true,
            reverse: true,
            format: DurationFormat::HourMinSec,
        };
        assert!(countdown.remaining() > Duration::seconds(4));
    }
}
