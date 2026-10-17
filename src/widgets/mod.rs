pub mod providers;

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span, Text},
    widgets::{Paragraph, Widget, Wrap},
};

use crate::config::WidgetConfig;
use crate::themes::Theme;

use providers::run_widget;

/// How many widget columns fit, based on how wide the terminal is compared to
/// its height. Terminal cells are roughly twice as tall as they are wide.
/// The least a widget column can be and still show a title and a line of text.
const MIN_WIDGET_HEIGHT: u16 = 3;
const SQUARE_COLUMNS: usize = 2;
const WIDE_COLUMNS: usize = 4;
const ULTRAWIDE_COLUMNS: usize = 6;
const CELL_HEIGHT_TO_WIDTH: f32 = 2.0;
const WIDE_ASPECT: f32 = 1.5;
const ULTRAWIDE_ASPECT: f32 = 2.2;

struct Item {
    config: WidgetConfig,
    title: String,
    refresh: Duration,
    timeout: Duration,
    output: String,
    running: bool,
    rerun: bool,
    visible: bool,
    scroll: u16,
    next_run: Instant,
    handle: Option<JoinHandle<()>>,
}

struct Message {
    index: usize,
    output: String,
}

/// Runs every configured widget on its own schedule and lays them out in a row
/// below the clock.
///
/// Each widget owns a worker thread while it runs, and results come back over a
/// channel so the draw loop never blocks on a command. Widgets that are not on
/// screen are not scheduled at all, so a hidden widget costs nothing.
pub struct Widgets {
    items: Vec<Item>,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    cancel: Arc<AtomicBool>,
    viewports: Vec<Rect>,
    active: Option<usize>,
    max_columns: Option<usize>,
    hidden: bool,
}

impl Widgets {
    pub fn new(configs: Vec<WidgetConfig>, max_columns: Option<usize>) -> Self {
        let (tx, rx) = mpsc::channel();
        let now = Instant::now();

        let items = configs
            .into_iter()
            .map(|config| {
                let title = config
                    .title
                    .clone()
                    .unwrap_or_else(|| default_title(&config));

                Item {
                    refresh: duration_or_default(config.refresh_secs, 900),
                    timeout: duration_or_default(config.timeout_secs, 30),
                    title,
                    config,
                    output: "Carregando...".to_string(),
                    running: false,
                    rerun: false,
                    visible: false,
                    scroll: 0,
                    next_run: now,
                    handle: None,
                }
            })
            .collect();

        Self {
            items,
            tx,
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
            viewports: Vec::new(),
            active: None,
            max_columns,
            hidden: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn hidden(&self) -> bool {
        self.hidden
    }

    /// `z` hides every widget so the clock can own the whole terminal.
    pub fn toggle_hidden(&mut self) -> bool {
        self.hidden = !self.hidden;
        if self.hidden {
            self.hide_all();
        }
        self.hidden
    }

    pub fn hide_all(&mut self) {
        for item in &mut self.items {
            item.visible = false;
        }
        self.viewports
            .iter_mut()
            .for_each(|area| *area = Rect::default());
        self.active = None;
    }

    /// Marks every visible widget for an immediate refresh, used when the theme
    /// changes so themed output is not stale until the next interval.
    pub fn reload(&mut self) {
        let now = Instant::now();
        for item in &mut self.items {
            if item.visible {
                item.output = "Carregando...".to_string();
                item.scroll = 0;
            }
            if item.running {
                item.rerun = true;
            } else if item.visible {
                item.next_run = now;
            }
        }
    }

    pub fn tick(&mut self, theme_name: &str) {
        let now = Instant::now();
        let mut finished = Vec::new();

        while let Ok(message) = self.rx.try_recv() {
            if let Some(item) = self.items.get_mut(message.index) {
                if !item.rerun {
                    item.output = message.output;
                }
                item.clamp_scroll_to_output();
                item.running = false;
                item.next_run = if item.rerun {
                    item.rerun = false;
                    now
                } else {
                    now + item.refresh
                };
                if let Some(handle) = item.handle.take() {
                    finished.push(handle);
                }
            }
        }
        for handle in finished {
            let _ = handle.join();
        }

        for (index, item) in self.items.iter_mut().enumerate() {
            if self.hidden || !item.visible || item.running || now < item.next_run {
                continue;
            }

            item.running = true;
            let config = item.config.clone();
            let timeout = item.timeout;
            let theme = theme_name.to_string();
            let tx = self.tx.clone();
            let cancel = self.cancel.clone();

            item.handle = Some(thread::spawn(move || {
                let output = run_widget(&config, &theme, timeout, cancel);
                let _ = tx.send(Message { index, output });
            }));
        }
    }

    pub fn render(&mut self, area: Rect, terminal: Rect, buf: &mut Buffer, theme: Theme) {
        self.viewports = vec![Rect::default(); self.items.len()];
        for item in &mut self.items {
            item.visible = false;
        }
        if self.hidden || area.width == 0 || area.height == 0 {
            return;
        }

        let columns = columns_to_show(
            terminal,
            self.items.len(),
            self.max_columns.unwrap_or(usize::MAX),
        );
        if columns == 0 {
            return;
        }

        // Splitting the terminal drops columns, so the widgets that no longer
        // fit get another row instead of disappearing: a narrower pane shows
        // fewer columns and more rows, and every configured widget stays on
        // screen.
        let rows_needed = self.items.len().div_ceil(columns);
        let rows_that_fit = (area.height / MIN_WIDGET_HEIGHT).max(1) as usize;
        let rows = rows_needed.min(rows_that_fit).max(1);
        let shown = (rows * columns).min(self.items.len());

        // Each row is as tall as its tallest widget, so two rows do not leave a
        // lake of empty space between them.
        let column_width = (area.width / columns as u16).saturating_sub(2).max(1);
        let mut heights = vec![MIN_WIDGET_HEIGHT; rows];
        for (row, height) in heights.iter_mut().enumerate() {
            for column in 0..columns {
                let index = row * columns + column;
                if index >= shown {
                    break;
                }
                let item = &self.items[index];
                let needed = text_height(&item.title, &item.output, column_width).saturating_add(2);
                *height = (*height).max(needed);
            }
        }

        // Content taller than the area falls back to equal bands, and each
        // widget then scrolls.
        let wanted: u16 = heights.iter().copied().fold(0, u16::saturating_add);
        let constraints = if wanted <= area.height {
            heights
                .iter()
                .map(|height| Constraint::Length(*height))
                .collect::<Vec<_>>()
        } else {
            (0..rows)
                .map(|_| Constraint::Ratio(1, rows as u32))
                .collect::<Vec<_>>()
        };

        let row_bands = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        for (row, band) in row_bands.iter().enumerate() {
            let column_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    (0..columns)
                        .map(|_| Constraint::Ratio(1, columns as u32))
                        .collect::<Vec<_>>(),
                )
                .split(*band);

            for (column, chunk) in column_chunks.iter().enumerate() {
                let index = row * columns + column;
                if index >= shown {
                    break;
                }

                let viewport = padded(*chunk);
                self.viewports[index] = viewport;
                let item = &mut self.items[index];
                item.visible = true;
                render_item(item, viewport, buf, theme);
            }
        }

        // When not even the extra rows fit, say what is left out instead of
        // leaving the reader to guess.
        if shown < self.items.len() {
            let rest = self.items.len() - shown;
            let banner = format!(" +{rest} painel(ns) sem espaço ");
            let banner = banner.chars().take(area.width as usize).collect::<String>();
            buf.set_string(
                area.x,
                area.bottom().saturating_sub(1),
                &banner,
                theme.header.add_modifier(Modifier::REVERSED),
            );
        }
    }

    pub fn scroll_at(&mut self, column: u16, row: u16, delta: i16) {
        if let Some(index) = self
            .viewports
            .iter()
            .position(|area| contains(*area, column, row))
        {
            self.active = Some(index);
            if let Some(item) = self.items.get_mut(index) {
                item.scroll_by(delta);
            }
        }
    }

    /// Keyboard scrolling acts on the last widget touched by the mouse, or the
    /// first visible widget so `Home` and `End` work without any pointer input.
    fn scroll_target(&self) -> Option<usize> {
        self.active
            .filter(|&index| self.items.get(index).is_some_and(|item| item.visible))
            .or_else(|| self.items.iter().position(|item| item.visible))
    }

    pub fn scroll_to_top(&mut self) {
        if let Some(item) = self
            .scroll_target()
            .and_then(|index| self.items.get_mut(index))
        {
            item.scroll = 0;
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        if let Some(index) = self.scroll_target()
            && let (Some(item), Some(area)) = (self.items.get_mut(index), self.viewports.get(index))
        {
            item.scroll = item.max_scroll(*area);
        }
    }

    pub fn scroll_by_page(&mut self, pages: i16) {
        if let Some(index) = self.scroll_target() {
            self.active = Some(index);
            if let (Some(item), Some(area)) = (self.items.get_mut(index), self.viewports.get(index))
            {
                let page = i16::try_from(area.height).unwrap_or(i16::MAX).max(1);
                item.scroll_by(pages.saturating_mul(page));
                item.clamp_scroll(*area);
            }
        }
    }
}

impl Drop for Widgets {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        for item in &mut self.items {
            if let Some(handle) = item.handle.take() {
                let _ = handle.join();
            }
        }
    }
}

/// Runs one widget once, synchronously, for --check and other tooling.
pub fn run_once(config: &WidgetConfig, theme: &str, timeout: Duration) -> String {
    run_widget(config, theme, timeout, Arc::new(AtomicBool::new(false)))
}

impl Item {
    /// Height the widget needs for its full output at `width`, so the scroll
    /// clamp knows how far down the content goes.
    fn max_scroll(&self, area: Rect) -> u16 {
        text_height(&self.title, &self.output, area.width).saturating_sub(area.height)
    }

    fn clamp_scroll(&mut self, area: Rect) {
        let max = self.max_scroll(area);
        self.scroll = self.scroll.min(max);
    }

    fn clamp_scroll_to_output(&mut self) {
        let lines = text_height(&self.title, &self.output, 1);
        self.scroll = self.scroll.min(lines.saturating_sub(1));
    }

    fn scroll_by(&mut self, delta: i16) {
        if delta.is_negative() {
            self.scroll = self.scroll.saturating_sub(delta.unsigned_abs());
        } else {
            self.scroll = self.scroll.saturating_add(delta as u16);
        }
    }
}

fn render_item(item: &Item, area: Rect, buf: &mut Buffer, theme: Theme) {
    Paragraph::new(widget_text(&item.title, &item.output, theme))
        .style(theme.widget)
        .scroll((item.scroll, 0))
        .wrap(Wrap { trim: false })
        .render(area, buf);
}

fn widget_text(title: &str, output: &str, theme: Theme) -> Text<'static> {
    let mut lines = Vec::new();

    // An empty title suppresses the header line, so a widget whose command
    // prints its own heading owns the whole area.
    if !title.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("● ", theme.widget_marker),
            Span::styled(title.to_string(), theme.widget_title),
        ]));
    }

    for line in sanitize(output).lines() {
        lines.push(Line::styled(line.to_string(), theme.widget));
    }

    Text::from(lines)
}

fn text_height(title: &str, output: &str, width: u16) -> u16 {
    let width = width.max(1) as usize;
    let mut total = usize::from(!title.is_empty());

    for line in sanitize(output).lines() {
        total = total.saturating_add(line.chars().count().max(1).div_ceil(width));
    }

    total.min(u16::MAX as usize) as u16
}

/// Command output is printed as-is, minus terminal control sequences: a widget
/// that emits color codes must not be able to scramble the screen.
fn sanitize(output: &str) -> String {
    let mut result = String::with_capacity(output.len());
    let mut chars = output.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\x1b' => match chars.peek() {
                Some('[') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if ('@'..='~').contains(&next) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '\x07' {
                            break;
                        }
                        if next == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\t' => result.push_str("    "),
            '\n' => result.push('\n'),
            c if !c.is_control() => result.push(c),
            _ => {}
        }
    }

    result
}

fn columns_to_show(terminal: Rect, configured: usize, max_columns: usize) -> usize {
    let height = terminal.height.max(1) as f32;
    let aspect = terminal.width as f32 / (height * CELL_HEIGHT_TO_WIDTH);
    let fitting = if aspect < WIDE_ASPECT {
        SQUARE_COLUMNS
    } else if aspect < ULTRAWIDE_ASPECT {
        WIDE_COLUMNS
    } else {
        ULTRAWIDE_COLUMNS
    };

    configured.min(fitting).min(max_columns)
}

fn padded(area: Rect) -> Rect {
    let x = u16::from(area.width > 2);
    let y = u16::from(area.height > 2);

    Rect {
        x: area.x + x,
        y: area.y + y,
        width: area.width.saturating_sub(x * 2),
        height: area.height.saturating_sub(y * 2),
    }
}

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

fn duration_or_default(secs: u64, default: u64) -> Duration {
    Duration::from_secs(if secs == 0 { default } else { secs })
}

fn default_title(config: &WidgetConfig) -> String {
    match config.kind.trim().to_ascii_lowercase().as_str() {
        "cpu" => "CPU".to_string(),
        "mem" | "ram" => "RAM".to_string(),
        "weather" => "Clima".to_string(),
        "gh" | "github" => "GitHub".to_string(),
        "command" => config
            .command
            .first()
            .cloned()
            .unwrap_or_else(|| "comando".to_string()),
        _ => config.kind.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(kind: &str, title: Option<&str>) -> WidgetConfig {
        WidgetConfig {
            kind: kind.to_string(),
            title: title.map(str::to_string),
            command: Vec::new(),
            refresh_secs: 900,
            timeout_secs: 30,
            weather: Default::default(),
            gh: Default::default(),
            git: Default::default(),
            http: Default::default(),
            disk: Default::default(),
            net: Default::default(),
            commits: Default::default(),
        }
    }

    #[test]
    fn titles_fall_back_to_the_widget_kind() {
        assert_eq!(default_title(&config("cpu", None)), "CPU");
        assert_eq!(default_title(&config("mem", None)), "RAM");
        assert_eq!(default_title(&config("weather", None)), "Clima");
    }

    #[test]
    fn an_explicit_title_wins() {
        let widgets = Widgets::new(vec![config("cpu", Some("Processador"))], None);
        assert_eq!(widgets.items[0].title, "Processador");
    }

    #[test]
    fn an_empty_title_is_kept_so_the_header_can_be_suppressed() {
        let widgets = Widgets::new(vec![config("cpu", Some(""))], None);
        assert_eq!(widgets.items[0].title, "");
    }

    #[test]
    fn the_number_of_columns_follows_the_terminal_shape() {
        let square = Rect::new(0, 0, 80, 40);
        assert_eq!(columns_to_show(square, 10, usize::MAX), 2);

        // 140x40 tem aspecto 1,75: largo, mas não ultrawide.
        let wide = Rect::new(0, 0, 140, 40);
        assert_eq!(columns_to_show(wide, 10, usize::MAX), 4);

        let ultrawide = Rect::new(0, 0, 400, 40);
        assert_eq!(columns_to_show(ultrawide, 10, usize::MAX), 6);
    }

    #[test]
    fn there_are_never_more_columns_than_widgets_or_than_the_configured_cap() {
        let ultrawide = Rect::new(0, 0, 400, 40);
        assert_eq!(columns_to_show(ultrawide, 1, usize::MAX), 1);
        assert_eq!(columns_to_show(ultrawide, 10, 3), 3);
        assert_eq!(columns_to_show(ultrawide, 0, usize::MAX), 0);
    }

    #[test]
    fn text_height_counts_wrapped_lines_and_the_title() {
        assert_eq!(text_height("", "uma linha", 40), 1);
        assert_eq!(text_height("CPU", "uma linha", 40), 2);
        // Ten characters in a five wide area wrap onto two lines.
        assert_eq!(text_height("", "0123456789", 5), 2);
    }

    #[test]
    fn sanitize_drops_control_sequences_but_keeps_text() {
        assert_eq!(sanitize("\x1b[31mvermelho\x1b[0m"), "vermelho");
        assert_eq!(sanitize("a\tb"), "a    b");
        assert_eq!(sanitize("linha1\nlinha2"), "linha1\nlinha2");
        assert_eq!(sanitize("\x1b]0;titulo\x07ok"), "ok");
    }

    #[test]
    fn hiding_widgets_clears_the_visible_flags() {
        let mut widgets = Widgets::new(vec![config("cpu", None)], None);
        widgets.items[0].visible = true;

        assert!(widgets.toggle_hidden(), "o primeiro z esconde");
        assert!(!widgets.items[0].visible);

        assert!(!widgets.toggle_hidden(), "o segundo z mostra de novo");
    }
}
