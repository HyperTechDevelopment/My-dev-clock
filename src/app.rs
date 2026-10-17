use std::path::PathBuf;
use std::time::Instant;

use chrono::{
    DateTime, Duration as ChronoDuration, Local, LocalResult, NaiveDate, NaiveDateTime, NaiveTime,
    TimeZone,
};
use chrono_tz::Tz;
use clap::{Parser, Subcommand};
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Clear, Paragraph, Widget},
};

use crate::animation::{self, ANIMATIONS, Animation};
use crate::clock_text::{FONTS, FontStyle, clock_height, fit_scale};
use crate::config::{self, ClockConfig, Config, DisplayConfig};
use crate::modes::{
    self, Clock, ClockOptions, Countdown, DurationFormat, FlipState, Pause, Stopwatch, Timer,
    TimerOptions,
};
use crate::size_input::{ScaleAction, ScaleInput};
use crate::themes::{self, Theme};
use crate::widgets::Widgets;

#[derive(Parser, Debug)]
#[command(
    name = "my-dev-clock",
    version,
    about = "Relógio de terminal com timer, cronômetro e widgets",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub mode: Option<Mode>,

    /// Cor base dos dígitos: um nome (green, cyan, lightred, ...) ou "#RRGGBB"
    #[arg(short, long, global = true, value_parser = themes::parse_color)]
    pub color: Option<Color>,

    /// Escala dos dígitos, de 1 a 12. Sem esta opção o relógio se ajusta à janela.
    #[arg(short, long, global = true, value_parser = parse_scale)]
    pub size: Option<u16>,

    /// Tema inicial: default, matrix, neon, system, fire ou ice
    #[arg(long, global = true, value_parser = parse_theme)]
    pub theme: Option<String>,

    /// Caminho de um arquivo de configuração específico
    #[arg(long, global = true, value_name = "ARQUIVO")]
    pub config: Option<PathBuf>,

    /// Estilo dos dígitos: bricks, slim, slant ou flip
    #[arg(long, global = true, value_parser = parse_font)]
    pub font: Option<FontStyle>,

    /// Animação de fundo: none, matrix, plasma, waves ou stars
    #[arg(long, global = true, value_parser = parse_animation)]
    pub animation: Option<Animation>,

    /// O relógio ocupa a área toda em vez de só a metade de cima
    #[arg(long, global = true, overrides_with = "no_fill")]
    pub fill: bool,

    /// O relógio volta a ficar na metade de cima
    #[arg(long, global = true, overrides_with = "fill")]
    pub no_fill: bool,

    /// Imprime uma linha e sai, para barra de status ou prompt. Marcadores:
    /// {time} {date} {git} {cpu} {mem} {host}
    #[arg(long, value_name = "FORMATO", num_args = 0..=1, default_missing_value = "{time}")]
    pub print: Option<String>,

    /// Grava um arquivo de configuração de exemplo no local padrão e sai
    #[arg(long)]
    pub init: bool,

    /// Testa a configuração sem abrir a interface: mostra de onde ela veio e roda
    /// cada widget uma vez, imprimindo o que ele devolve
    #[arg(long)]
    pub check: bool,

    #[command(flatten)]
    pub display: DisplayArgs,
}

#[derive(Debug, Subcommand)]
pub enum Mode {
    /// Mostra a hora atual. É o modo padrão.
    Clock {
        /// Fuso horário, por exemplo "America/Sao_Paulo"
        #[arg(long, value_parser = parse_timezone)]
        timezone: Option<Tz>,
    },
    /// Conta o tempo restante até o fim do timer
    Timer {
        /// Duração inicial. Aceita vários valores para correr em sequência, ex.: 25m 5m
        #[arg(short, long = "duration", value_parser = parse_duration, num_args = 1..)]
        durations: Vec<ChronoDuration>,

        /// Reinicia o timer quando ele termina
        #[arg(long, short, action)]
        repeat: bool,

        /// Encerra o app quando o tempo acaba
        #[arg(long, short = 'Q', action)]
        auto_quit: bool,

        /// Comando executado quando o tempo acaba
        #[arg(long, short, num_args = 1.., allow_hyphen_values = true)]
        execute: Vec<String>,
    },
    /// Mostra o tempo decorrido desde o início
    Stopwatch,
    /// Conta o tempo restante até um horário específico
    Countdown {
        /// Alvo, ex.: "2026-01-01", "20:00" ou "2026-12-25 20:00:00"
        #[arg(long, short, value_parser = parse_datetime)]
        time: DateTime<Local>,

        /// Continua contando depois de passar do alvo
        #[arg(long = "continue", short = 'C', action)]
        continue_on_zero: bool,

        /// Conta para cima em vez de para baixo
        #[arg(long, action)]
        reverse: bool,
    },
}

/// Opções de exibição aceitas por todos os modos, antes ou depois do nome do modo.
#[derive(clap::Args, Debug, Default, Clone, PartialEq, Eq)]
pub struct DisplayArgs {
    /// Título mostrado acima dos dígitos. No timer vale um título por duração.
    #[arg(short = 'T', long = "title", value_name = "TITULO", num_args = 1.., action = clap::ArgAction::Append, global = true)]
    pub titles: Vec<String>,

    /// Mostra os segundos
    #[arg(long, action, global = true, overrides_with = "no_seconds")]
    pub seconds: bool,

    /// Esconde os segundos, deixando todo modo em horas e minutos
    #[arg(short = 'S', long, action, global = true, overrides_with = "seconds")]
    pub no_seconds: bool,

    /// Mostra os décimos de segundo
    #[arg(short = 'm', long, action, global = true, overrides_with = "no_millis")]
    pub millis: bool,

    /// Esconde os décimos de segundo
    #[arg(short = 'M', long, action, global = true, overrides_with = "millis")]
    pub no_millis: bool,

    /// Mostra a linha de data (modo relógio)
    #[arg(long, action, global = true, overrides_with = "no_date")]
    pub date: bool,

    /// Esconde a linha de data (modo relógio)
    #[arg(short = 'D', long, action, global = true, overrides_with = "date")]
    pub no_date: bool,

    /// Começa pausado (timer e cronômetro)
    #[arg(short = 'P', long, action, global = true)]
    pub paused: bool,
}

impl DisplayArgs {
    fn show_seconds(&self) -> Option<bool> {
        flag_pair(self.seconds, self.no_seconds)
    }

    fn show_millis(&self) -> Option<bool> {
        flag_pair(self.millis, self.no_millis)
    }

    fn show_date(&self) -> Option<bool> {
        flag_pair(self.date, self.no_date)
    }

    fn start_paused(&self) -> Option<bool> {
        self.paused.then_some(true)
    }
}

fn flag_pair(on: bool, off: bool) -> Option<bool> {
    if on {
        Some(true)
    } else if off {
        Some(false)
    } else {
        None
    }
}

/// What one mode shows when neither the command line nor the config says
/// otherwise.
#[derive(Debug, Clone, Copy)]
struct ModeDefaults {
    show_date: bool,
    show_seconds: bool,
    show_millis: bool,
}

const CLOCK_DEFAULTS: ModeDefaults = ModeDefaults {
    show_date: true,
    show_seconds: true,
    show_millis: false,
};
const TIMER_DEFAULTS: ModeDefaults = ModeDefaults {
    show_date: false,
    show_seconds: true,
    show_millis: true,
};
const STOPWATCH_DEFAULTS: ModeDefaults = ModeDefaults {
    show_date: false,
    show_seconds: true,
    show_millis: true,
};
const COUNTDOWN_DEFAULTS: ModeDefaults = ModeDefaults {
    show_date: false,
    show_seconds: true,
    show_millis: false,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct DisplayOptions {
    titles: Vec<String>,
    show_date: bool,
    show_seconds: bool,
    show_millis: bool,
    start_paused: bool,
}

impl DisplayOptions {
    /// Precedence, highest first: command-line flag, the mode's config section,
    /// `[default]`, then the built-in default for that mode.
    fn resolve(
        args: &DisplayArgs,
        mode: Option<&DisplayConfig>,
        default: Option<&DisplayConfig>,
        builtin: ModeDefaults,
        mode_titles: Vec<String>,
    ) -> Self {
        let pick = |cli: Option<bool>, get: fn(&DisplayConfig) -> Option<bool>, fallback: bool| {
            cli.or_else(|| mode.and_then(get))
                .or_else(|| default.and_then(get))
                .unwrap_or(fallback)
        };

        let titles = if !args.titles.is_empty() {
            args.titles.clone()
        } else if !mode_titles.is_empty() {
            mode_titles
        } else {
            mode.and_then(|config| config.title.clone())
                .or_else(|| default.and_then(|config| config.title.clone()))
                .into_iter()
                .collect()
        };

        Self {
            titles,
            show_date: pick(args.show_date(), |c| c.show_date, builtin.show_date),
            show_seconds: pick(
                args.show_seconds(),
                |c| c.show_seconds,
                builtin.show_seconds,
            ),
            show_millis: pick(args.show_millis(), |c| c.show_millis, builtin.show_millis),
            start_paused: pick(args.start_paused(), |c| c.start_paused, false),
        }
    }

    fn title(&self) -> Option<String> {
        self.titles.first().cloned()
    }

    fn duration_format(&self) -> DurationFormat {
        DurationFormat::from_display(self.show_seconds, self.show_millis)
    }
}

/// What the main loop should do after a key press.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
}

/// The key map shown by `?`. It lives next to the handler so the two stay in
/// step, and a test checks it against the list the README promises.
const HELP_LINES: [&str; 19] = [
    "q           sai",
    "espaço      pausa e retoma (timer e cronômetro)",
    "c / w / t   relógio / cronômetro / timer",
    "Shift+T     próximo tema",
    "Shift+F     próximo estilo de dígitos",
    "Shift+A     próxima animação de fundo",
    "Shift+L     tela cheia sim/não",
    "z           esconde ou mostra os widgets",
    "s           digita o tamanho dos dígitos",
    "?           esta ajuda",
    "",
    "l           marca uma volta (cronômetro)",
    "n           pula para o próximo intervalo (timer)",
    "+ / -       soma ou tira um minuto (timer)",
    "r           reinicia o timer",
    "",
    "Home / End  topo ou fim do widget em foco",
    "PgUp / PgDn rola o widget em foco",
    "roda do mouse rola o widget sob o cursor",
];

/// The look settings taken from the command line, kept so a reload of the
/// config file can re-apply the same precedence.
#[derive(Debug, Default, Clone)]
struct LookOverrides {
    color: Option<Color>,
    size: Option<u16>,
    theme: Option<String>,
    font: Option<FontStyle>,
    animation: Option<Animation>,
    fill: Option<bool>,
}

/// A short confirmation of what a shortcut just changed, drawn for a moment at
/// the bottom of the screen so no key press is a guess.
#[derive(Debug)]
struct Status {
    text: String,
    shown_at: Instant,
}

pub struct App {
    config: Config,
    config_path: Option<PathBuf>,
    display_args: DisplayArgs,
    mode: String,
    clock: Option<Clock>,
    timer: Option<Timer>,
    stopwatch: Option<Stopwatch>,
    countdown: Option<Countdown>,
    widgets: Widgets,
    color: Color,
    scale: Option<u16>,
    scale_input: Option<ScaleInput>,
    display: DisplayOptions,
    themes: Vec<String>,
    theme_index: usize,
    font: FontStyle,
    fill: bool,
    animation: Animation,
    animation_fps: u32,
    /// How long a digit takes to flip, zero when the animation is off.
    flip_ms: u64,
    flips: FlipState,
    started: Instant,
    status: Option<Status>,
    /// Whether the help overlay is open.
    help: bool,
    /// The config file timestamp seen last, used to notice edits.
    config_stamp: Option<std::time::SystemTime>,
    /// The countdown rings only once per target.
    countdown_bell: bool,
    countdown_fired: bool,
    look: LookOverrides,
}

impl App {
    pub fn from_cli(cli: Cli) -> Self {
        let (config, config_path) = config::load(cli.config.as_deref());

        let mode = match &cli.mode {
            Some(Mode::Timer { .. }) => "timer".to_string(),
            Some(Mode::Stopwatch) => "stopwatch".to_string(),
            Some(Mode::Countdown { .. }) => "countdown".to_string(),
            Some(Mode::Clock { .. }) => "clock".to_string(),
            None => normalize_mode(&config.default.mode),
        };

        let mut app = Self {
            config,
            config_path,
            look: LookOverrides {
                color: cli.color,
                size: cli.size,
                theme: cli.theme.clone(),
                font: cli.font,
                animation: cli.animation,
                fill: flag_pair(cli.fill, cli.no_fill),
            },
            display_args: cli.display.clone(),
            mode,
            clock: None,
            timer: None,
            stopwatch: None,
            countdown: None,
            widgets: Widgets::new(Vec::new(), None),
            color: Color::Green,
            scale: None,
            scale_input: None,
            display: DisplayOptions {
                titles: Vec::new(),
                show_date: false,
                show_seconds: true,
                show_millis: false,
                start_paused: false,
            },
            themes: Vec::new(),
            theme_index: 0,
            font: FontStyle::Bricks,
            fill: false,
            animation: Animation::None,
            animation_fps: DEFAULT_ANIMATION_FPS,
            flip_ms: 0,
            flips: FlipState::default(),
            started: Instant::now(),
            status: None,
            help: false,
            config_stamp: None,
            countdown_bell: false,
            countdown_fired: false,
        };

        app.apply_look();
        app.remember_config_stamp();
        app.init_mode(cli.mode);
        app
    }

    /// Resolves the whole appearance from the config plus the command line
    /// overrides. Called on start and again after every config reload, so the
    /// two paths can never drift apart.
    fn apply_look(&mut self) {
        self.color = self
            .look
            .color
            .or_else(|| themes::parse_color(&self.config.default.color).ok())
            .unwrap_or(Color::Green);
        self.scale = self
            .look
            .size
            .or(self.config.default.size)
            .filter(|scale| *scale > 0);

        let current = self.theme_name().to_string();
        let mut names = themes::cycle_order(&self.config.clock.themes, self.look.theme.as_deref());
        if self.config.theme.custom.is_some() && !names.iter().any(|name| themes::is_custom(name)) {
            names.push(themes::CUSTOM.to_string());
        }
        self.theme_index = names.iter().position(|name| *name == current).unwrap_or(0);
        self.themes = names;

        self.font = font_from_config(self.look.font, &self.config.clock);
        self.animation = animation_from_config(self.look.animation, &self.config.clock);
        self.fill = self.look.fill.or(self.config.clock.fill).unwrap_or(false);
        self.animation_fps = self
            .config
            .clock
            .animation_fps
            .unwrap_or(DEFAULT_ANIMATION_FPS)
            .clamp(1, 60);
        self.refresh_flip();

        // A fresh set of workers: the previous ones are drained on drop.
        self.widgets = Widgets::new(
            self.config.clock.widgets.clone(),
            self.config.clock.max_columns,
        );
    }

    /// Records the config file timestamp, so the next edit can be noticed.
    fn remember_config_stamp(&mut self) {
        self.config_stamp = self.config_path.as_ref().and_then(config_stamp);
    }

    /// Fills the placeholders of --print. An unknown placeholder is left
    /// alone, so a typo shows up instead of turning into an empty line.
    pub fn print_line(&self, format: &str) -> String {
        let config = &self.config.clock;
        let clock = Clock::new(ClockOptions {
            title: None,
            show_date: true,
            show_seconds: true,
            show_millis: false,
            timezone: config.timezone,
            format: clock_format(config),
            date_format: date_format(config),
            world: world_zones(config),
        });

        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "?".to_string());

        let mut line = format.to_string();
        for (token, value) in [
            ("{time}", clock.text()),
            ("{date}", clock.header().unwrap_or_default()),
            ("{host}", host),
            ("{git}", crate::widgets::providers::git_branch_token(None)),
            ("{cpu}", crate::widgets::providers::cpu_line()),
            ("{mem}", crate::widgets::providers::memory_line()),
        ] {
            line = line.replace(token, &value);
        }
        line
    }

    /// Reloads the config when the file changed on disk, which makes tuning a
    /// theme or a widget a matter of saving the file.
    fn reload_config_if_changed(&mut self) {
        let Some(path) = self.config_path.clone() else {
            return;
        };
        let stamp = config_stamp(&path);
        if stamp.is_none() || stamp == self.config_stamp {
            return;
        }

        self.config_stamp = stamp;
        let (config, _) = config::load(Some(&path));
        self.config = config;
        self.apply_look();

        // A running timer or stopwatch keeps counting; the clock is rebuilt
        // because its look comes from the config.
        if self.mode == "clock" {
            self.init_mode(None);
        }
        self.announce("configuração recarregada".to_string());
    }

    pub fn config_path(&self) -> Option<&PathBuf> {
        self.config_path.as_ref()
    }

    /// Builds the active mode from the command line plus the config, and drops
    /// the other modes along with any worker threads they were holding.
    fn init_mode(&mut self, cli_mode: Option<Mode>) {
        self.clock = None;
        self.timer = None;
        self.stopwatch = None;
        self.countdown = None;

        let default_display = Some(&self.config.default.display);
        let active = self.mode.clone();
        // A display flag given on the command line keeps applying after a mode
        // switch, so it is re-resolved against the new mode's config.
        let args = self.display_args.clone();

        match (active.as_str(), cli_mode) {
            ("clock", cli_mode) => {
                let timezone = match cli_mode {
                    Some(Mode::Clock { timezone }) => timezone,
                    _ => None,
                };
                let config = &self.config.clock;
                self.display = DisplayOptions::resolve(
                    &args,
                    Some(&config.display),
                    default_display,
                    CLOCK_DEFAULTS,
                    Vec::new(),
                );
                self.clock = Some(Clock::new(ClockOptions {
                    title: self.display.title(),
                    show_date: self.display.show_date,
                    show_seconds: self.display.show_seconds,
                    show_millis: self.display.show_millis,
                    timezone: timezone.or(config.timezone),
                    format: clock_format(config),
                    date_format: date_format(config),
                    world: world_zones(config),
                }));
            }
            ("timer", cli_mode) => {
                let (durations, repeat, auto_quit, execute) = match cli_mode {
                    Some(Mode::Timer {
                        durations,
                        repeat,
                        auto_quit,
                        execute,
                    }) => (durations, repeat, auto_quit, execute),
                    _ => (Vec::new(), false, false, Vec::new()),
                };
                let config = &self.config.timer;
                self.display = DisplayOptions::resolve(
                    &args,
                    Some(&config.display),
                    default_display,
                    TIMER_DEFAULTS,
                    config.titles.clone(),
                );

                let durations = if durations.is_empty() {
                    configured_durations(&config.durations)
                } else {
                    durations
                };
                let execute = if execute.is_empty() {
                    config.execute.clone()
                } else {
                    execute
                };

                self.timer = Some(Timer::new(TimerOptions {
                    durations,
                    titles: self.display.titles.clone(),
                    repeat: repeat || config.repeat,
                    format: self.display.duration_format(),
                    paused: self.display.start_paused,
                    auto_quit: auto_quit || config.auto_quit,
                    execute,
                    bell: config.bell,
                }));
            }
            ("stopwatch", _) => {
                let config = &self.config.stopwatch;
                self.display = DisplayOptions::resolve(
                    &args,
                    Some(&config.display),
                    default_display,
                    STOPWATCH_DEFAULTS,
                    Vec::new(),
                );
                self.stopwatch = Some(Stopwatch::new(
                    self.display.title(),
                    self.display.duration_format(),
                    self.display.start_paused,
                ));
            }
            ("countdown", cli_mode) => {
                let (time, continue_on_zero, reverse) = match cli_mode {
                    Some(Mode::Countdown {
                        time,
                        continue_on_zero,
                        reverse,
                    }) => (Some(time), continue_on_zero, reverse),
                    _ => (None, false, false),
                };
                let config = &self.config.countdown;
                self.display = DisplayOptions::resolve(
                    &args,
                    Some(&config.display),
                    default_display,
                    COUNTDOWN_DEFAULTS,
                    Vec::new(),
                );

                let target = time.or_else(|| {
                    config
                        .time
                        .as_deref()
                        .and_then(|time| parse_datetime(time).ok())
                });

                if let Some(target) = target {
                    self.countdown_bell = config.bell;
                    self.countdown_fired = false;
                    self.countdown = Some(Countdown {
                        time: target,
                        title: self.display.title(),
                        continue_on_zero: continue_on_zero || config.continue_on_zero,
                        reverse: reverse || config.reverse,
                        format: self.display.duration_format(),
                    });
                }
            }
            _ => {}
        }
    }

    pub fn theme_name(&self) -> &str {
        self.themes
            .get(self.theme_index)
            .map(String::as_str)
            .unwrap_or("default")
    }

    pub fn theme(&self) -> Theme {
        let base = Style::default().fg(self.color);
        let name = self.theme_name();

        match self.config.theme.custom.as_ref() {
            Some(spec) if themes::is_custom(name) => Theme::custom(spec, base),
            _ => Theme::named(name, base),
        }
    }

    fn cycle_theme(&mut self) {
        if self.themes.len() <= 1 {
            return;
        }
        self.theme_index = (self.theme_index + 1) % self.themes.len();
        // Themed widget output would otherwise stay stale until the next
        // interval, so refresh the visible ones right away.
        self.widgets.reload();

        let name = self.theme_name().to_string();
        self.announce(format!("tema: {name}"));
    }

    /// The next digit face in the built-in order.
    fn cycle_font(&mut self) {
        let current = FONTS
            .iter()
            .position(|font| *font == self.font)
            .unwrap_or(0);
        self.font = FONTS[(current + 1) % FONTS.len()];
        self.refresh_flip();

        self.announce(format!("fonte: {}", self.font.name()));
    }

    /// The next background animation in the built-in order.
    fn cycle_animation(&mut self) {
        let current = ANIMATIONS
            .iter()
            .position(|kind| *kind == self.animation)
            .unwrap_or(0);
        self.animation = ANIMATIONS[(current + 1) % ANIMATIONS.len()];

        self.announce(format!("animação: {}", self.animation.name()));
    }

    /// Switches between the clock owning the screen and sharing it.
    fn toggle_fill(&mut self) {
        self.fill = !self.fill;

        // Sem widgets na tela o relógio já ocupa tudo, e é justo dizer isso em
        // vez de deixar a impressão de que a tecla não fez nada.
        let state = if self.fill { "sim" } else { "não" };
        let note = if self.mode != "clock" {
            " (o relógio não está na tela)"
        } else if self.widgets.is_empty() || self.widgets.hidden() {
            " (sem widgets na tela, o relógio já usa tudo)"
        } else {
            ""
        };
        self.announce(format!("tela cheia: {state}{note}"));
    }

    /// Recomputes the flip timing. The face decides whether the animation is on
    /// unless the config states it explicitly.
    fn refresh_flip(&mut self) {
        let enabled = self.config.clock.flip.unwrap_or(self.font.is_flip());
        self.flip_ms = if enabled {
            self.config.clock.flip_ms.unwrap_or(DEFAULT_FLIP_MS)
        } else {
            0
        };
        self.flips = FlipState::new(self.flip_ms);
    }

    fn set_mode(&mut self, mode: &str) {
        self.mode = mode.to_string();
        self.init_mode(None);
    }

    fn pause_toggle(&mut self) {
        if let Some(timer) = self.timer.as_mut() {
            timer.toggle_paused();
        } else if let Some(stopwatch) = self.stopwatch.as_mut() {
            stopwatch.toggle_paused();
        }
    }

    pub fn tick(&mut self) {
        self.reload_config_if_changed();

        let theme_name = self.theme_name().to_string();
        self.widgets.tick(&theme_name);
        if let Some(timer) = self.timer.as_mut() {
            timer.tick(&theme_name);
        }

        // A contagem regressiva toca a campainha uma vez, ao chegar no alvo.
        if let Some(countdown) = self.countdown.as_ref()
            && !self.countdown_fired
            && countdown.reached()
        {
            self.countdown_fired = true;
            if self.countdown_bell {
                modes::ring();
            }
        }
    }

    pub fn is_ended(&self) -> bool {
        self.timer.as_ref().is_some_and(Timer::is_finished)
    }

    pub fn on_exit(&self) {
        let Some(stopwatch) = self.stopwatch.as_ref() else {
            return;
        };

        println!("Tempo do cronômetro: {}", stopwatch.summary());

        let mut previous = ChronoDuration::zero();
        for (index, total) in stopwatch.laps().iter().enumerate() {
            println!(
                "  volta {}: {}",
                index + 1,
                modes::format_duration(*total - previous, DurationFormat::HourMinSecDeci)
            );
            previous = *total;
        }

        // Timing a benchmark ends with the numbers in a file, not only on screen.
        if let Some(path) = self.config.stopwatch.laps_csv.as_deref()
            && !stopwatch.laps().is_empty()
        {
            match std::fs::write(path, stopwatch.laps_csv()) {
                Ok(()) => println!("Voltas gravadas em {path}"),
                Err(error) => eprintln!("aviso: não consegui gravar {path}: {error}"),
            }
        }
    }

    pub fn on_mouse_scroll(&mut self, column: u16, row: u16, kind: MouseEventKind) {
        match kind {
            MouseEventKind::ScrollUp => self.widgets.scroll_at(column, row, -1),
            MouseEventKind::ScrollDown => self.widgets.scroll_at(column, row, 1),
            _ => {}
        }
    }

    /// The appearance shortcuts, shared by the normal key path and the size
    /// prompt, so they behave the same in both.
    fn look_shortcut(&mut self, key: KeyCode) -> Option<Action> {
        match key {
            KeyCode::Char('T') => self.cycle_theme(),
            KeyCode::Char('F') => self.cycle_font(),
            KeyCode::Char('A') => self.cycle_animation(),
            KeyCode::Char('L') => self.toggle_fill(),
            _ => return None,
        }
        Some(Action::None)
    }

    /// Marks a lap on the stopwatch, which is what timing a benchmark needs
    /// beyond the total.
    fn lap(&mut self) {
        let Some(stopwatch) = self.stopwatch.as_mut() else {
            return;
        };

        stopwatch.lap();
        let number = stopwatch.laps().len();
        let total = stopwatch.summary();
        self.announce(format!("volta {number} · {total}"));
    }

    /// Jumps to the end of the timer's current interval.
    fn timer_skip(&mut self) {
        let Some(timer) = self.timer.as_mut() else {
            return;
        };

        timer.skip();
        let (remaining, index) = timer.remaining();
        let label = timer
            .titles
            .get(index)
            .cloned()
            .unwrap_or_else(|| format!("intervalo {}", index + 1));
        // A mensagem sai antes do anúncio: `announce` precisa de `&mut self`.
        let message = format!(
            "{label} · {} restante",
            modes::format_duration(remaining.max(ChronoDuration::zero()), timer.format)
        );
        self.announce(message);
    }

    /// Adds or removes a minute from the interval that is running.
    fn timer_adjust(&mut self, minutes: i64) {
        let Some(timer) = self.timer.as_mut() else {
            return;
        };

        timer.adjust_minutes(minutes);
        let (remaining, _) = timer.remaining();
        let direction = if minutes > 0 { "mais" } else { "menos" };
        let message = format!(
            "{direction} 1 min · {} restante",
            modes::format_duration(remaining.max(ChronoDuration::zero()), timer.format)
        );
        self.announce(message);
    }

    /// Puts the timer back at the first interval.
    fn timer_restart(&mut self) {
        let Some(timer) = self.timer.as_mut() else {
            return;
        };

        timer.restart();
        self.announce("timer reiniciado".to_string());
    }

    /// The whole key map, drawn over everything while it is open.
    fn render_help(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: Theme) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let widest = HELP_LINES
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0) as u16;
        // A pane too small for a box still deserves an answer to ?.
        if area.height < 3 || area.width < 16 {
            let hint = modes::truncate_to("? teclas: q sai · Esc fecha · Shift+T tema", area.width);
            buf.set_string(
                area.x,
                area.y.saturating_add(area.height.saturating_sub(1)),
                &hint,
                theme.header.add_modifier(Modifier::REVERSED),
            );
            return;
        }

        let width = (widest + 4).min(area.width);
        let height = (HELP_LINES.len() as u16 + 2).min(area.height);
        if width == 0 || height < 3 {
            return;
        }

        let popup = Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        };

        Clear.render(popup, buf);
        let block = Block::bordered()
            .title(" teclas · ? ou Esc fecha ")
            .title_style(theme.widget_title)
            .style(theme.widget);
        let inner = block.inner(popup);
        block.render(popup, buf);

        let lines: Vec<Line> = HELP_LINES
            .iter()
            .map(|line| Line::styled((*line).to_string(), theme.widget))
            .collect();
        Paragraph::new(Text::from(lines)).render(inner, buf);
    }

    /// Shows a one-line confirmation of the last shortcut, so the effect of a
    /// key is never something the eye has to infer.
    fn announce(&mut self, text: String) {
        self.status = Some(Status {
            text,
            shown_at: Instant::now(),
        });
    }

    fn render_status(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: Theme) {
        // The scale prompt owns the bottom line while it is open.
        if self.scale_input.is_some() || area.height == 0 {
            return;
        }
        let Some(status) = self.status.as_ref() else {
            return;
        };
        if status.shown_at.elapsed() >= STATUS_LINGER {
            return;
        }

        let width = (status.text.chars().count() as u16).min(area.width);
        if width == 0 {
            return;
        }
        let line = Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.bottom().saturating_sub(1),
            width,
            height: 1,
        };

        // Reversed video keeps it readable over any palette, animation or
        // widget text underneath.
        let style = theme.header.add_modifier(Modifier::REVERSED);
        buf.set_string(line.x, line.y, &status.text, style);
    }

    /// Handles one terminal event.
    ///
    /// Key releases are dropped on purpose: on Windows every key event carries a
    /// kind, so pressing and releasing both arrive, and acting on both applies
    /// every shortcut twice — a cycle then skips faces and a toggle ends up
    /// where it started.
    pub fn on_event(&mut self, event: Event) -> Action {
        match event {
            Event::Key(key) => {
                if key.kind == KeyEventKind::Release {
                    return Action::None;
                }
                self.on_key(key.code)
            }
            Event::Mouse(mouse) => {
                self.on_mouse_scroll(mouse.column, mouse.row, mouse.kind);
                Action::None
            }
            _ => Action::None,
        }
    }

    pub fn on_key(&mut self, key: KeyCode) -> Action {
        // O tamanho do campo de digitação não pode ser confundido com um
        // atalho de aparência, então estes quatro continuam valendo com o
        // campo aberto: engoli-los faria um "s" acidental parecer que todos
        // os atalhos pararam de funcionar.
        if let Some(action) = self.look_shortcut(key) {
            return action;
        }

        if let Some(input) = self.scale_input.as_mut() {
            match input.on_key(key) {
                // Preview applies the typed value as it grows, so the clock
                // resizes while the number is being typed.
                ScaleAction::Preview => {
                    if let Some(scale) = input.parse() {
                        self.scale = Some(scale);
                    }
                }
                ScaleAction::Committed(scale) => {
                    self.scale = Some(scale);
                    self.scale_input = None;
                }
                ScaleAction::Cancelled => self.scale_input = None,
                ScaleAction::Ignored => {}
            }
            return Action::None;
        }

        match key {
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Char(' ') => self.pause_toggle(),
            KeyCode::Char('c') => self.set_mode("clock"),
            KeyCode::Char('w') => self.set_mode("stopwatch"),
            KeyCode::Char('t') => self.set_mode("timer"),
            KeyCode::Char('z') => {
                self.widgets.toggle_hidden();
            }
            KeyCode::Char('s') => self.scale_input = Some(ScaleInput::open()),
            KeyCode::Char('?') => self.help = !self.help,
            KeyCode::Esc => self.help = false,
            // Voltas do cronômetro.
            KeyCode::Char('l') => self.lap(),
            // Controles do timer.
            KeyCode::Char('n') => self.timer_skip(),
            KeyCode::Char('r') => self.timer_restart(),
            KeyCode::Char('+') | KeyCode::Char('=') => self.timer_adjust(1),
            KeyCode::Char('-') => self.timer_adjust(-1),
            KeyCode::Home => self.widgets.scroll_to_top(),
            KeyCode::End => self.widgets.scroll_to_bottom(),
            KeyCode::PageUp => self.widgets.scroll_by_page(-1),
            KeyCode::PageDown => self.widgets.scroll_by_page(1),
            _ => {}
        }

        Action::None
    }

    /// Runs every configured widget once and prints what each one returns, so a
    /// widget that is not behaving can be diagnosed without the interface.
    pub fn check(&self) {
        match self.config_path.as_ref() {
            Some(path) => println!("configuração: {}", path.display()),
            None => println!("configuração: nenhuma encontrada, usando os padrões"),
        }
        println!("tema inicial: {}", self.theme_name());
        println!("temas no ciclo: {}", self.themes.join(", "));
        println!(
            "fonte: {} · animação: {} ({} fps) · tela cheia: {} · flip: {}",
            self.font.name(),
            self.animation.name(),
            self.animation_fps,
            if self.fill { "sim" } else { "não" },
            if self.flip_ms > 0 { "sim" } else { "não" }
        );

        println!(
            "formato: {} · data: {} · fusos extras: {}",
            self.config.clock.format.as_deref().unwrap_or("(padrão)"),
            self.config
                .clock
                .date_format
                .as_deref()
                .unwrap_or("(padrão)"),
            if self.config.clock.world.is_empty() {
                "nenhum".to_string()
            } else {
                self.config.clock.world.join(", ")
            }
        );
        println!(
            "tema custom: {} · campainha: {} · voltas em CSV: {}",
            if self.config.theme.custom.is_some() {
                "sim"
            } else {
                "não"
            },
            if self.config.timer.bell || self.config.countdown.bell {
                "sim"
            } else {
                "não"
            },
            self.config.stopwatch.laps_csv.as_deref().unwrap_or("(não)")
        );

        let widgets = &self.config.clock.widgets;
        if widgets.is_empty() {
            println!("\nnenhum widget configurado");
            return;
        }

        for (index, widget) in widgets.iter().enumerate() {
            let seconds = if widget.timeout_secs == 0 {
                30
            } else {
                widget.timeout_secs
            };
            let started = std::time::Instant::now();
            let output = crate::widgets::run_once(
                widget,
                self.theme_name(),
                std::time::Duration::from_secs(seconds),
            );
            println!(
                "\n[{}/{}] {} · {} · {:.1}s",
                index + 1,
                widgets.len(),
                widget.kind,
                widget
                    .title
                    .clone()
                    .unwrap_or_else(|| "título herdado".to_string()),
                started.elapsed().as_secs_f32()
            );
            println!("{output}");
        }
    }

    pub fn ui(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let theme = self.theme();
        let font = self.font;
        let buf = frame.buffer_mut();

        match self.mode.as_str() {
            "clock" => self.render_clock(area, buf, theme),
            "timer" => {
                self.paint_animation(area, buf, theme);
                if let Some(timer) = self.timer.as_ref() {
                    timer.flash(area, buf, theme);
                    let view = timer.view(self.scale, font, theme);
                    modes::render_view(area, buf, &view);
                }
            }
            "stopwatch" => {
                self.paint_animation(area, buf, theme);
                if let Some(stopwatch) = self.stopwatch.as_ref() {
                    let view = stopwatch.view(self.scale, font, theme);
                    modes::render_view(area, buf, &view);
                }
            }
            "countdown" => {
                self.paint_animation(area, buf, theme);
                if let Some(countdown) = self.countdown.as_ref()
                    && !countdown.hidden()
                {
                    let view = countdown.view(self.scale, font, theme);
                    modes::render_view(area, buf, &view);
                }
            }
            _ => {}
        }

        if let Some(input) = self.scale_input.as_ref() {
            input.render(area, buf, theme);
        } else {
            self.render_status(area, buf, theme);
        }

        if self.help {
            self.render_help(area, buf, theme);
        }
    }

    fn render_clock(&mut self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: Theme) {
        let Some(clock) = self.clock.as_ref() else {
            return;
        };

        let text = clock.text();
        let header = clock.header();
        let has_header = header.is_some();
        let shares_screen = !self.widgets.is_empty() && !self.widgets.hidden();

        // Alone, the clock owns the whole screen. Sharing it, the clock keeps
        // the top half, or everything the widget row does not need with `fill`.
        let room = if !shares_screen {
            area.height
        } else if self.fill {
            area.height.saturating_sub(WIDGET_ROW_MIN_HEIGHT).max(1)
        } else {
            (area.height / 2).max(1)
        };

        let scale = self.scale.unwrap_or_else(|| {
            fit_scale(
                &text,
                Rect {
                    height: room.saturating_sub(u16::from(has_header) * 2),
                    ..area
                },
                self.font,
            )
        });
        let digits = clock_height(self.font, scale, has_header)
            .min(room)
            .min(area.height)
            .max(1);

        // With `fill` the clock claims its whole room, so the digits and the
        // animation stretch across it instead of hugging the text.
        let clock_area = Rect {
            height: if !shares_screen {
                area.height
            } else if self.fill {
                room.min(area.height).max(digits)
            } else {
                digits
            },
            ..area
        };

        self.paint_animation(clock_area, buf, theme);

        let mut view = clock.view(Some(scale), self.font, theme);
        view.header = header;
        if self.flip_ms > 0 {
            view.flips = self.flips.update(&view.text, Instant::now());
        }
        modes::render_view(clock_area, buf, &view);

        if shares_screen {
            let widgets_area = Rect {
                y: area.y.saturating_add(clock_area.height),
                height: area.height.saturating_sub(clock_area.height),
                ..area
            };
            self.widgets.render(widgets_area, area, buf, theme);
        }
    }

    /// Paints the background animation, when one is configured.
    fn paint_animation(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: Theme) {
        animation::render(
            self.animation,
            area,
            buf,
            animation::AnimationFrame {
                dim: theme.animation,
                accent: theme.accent,
                elapsed: self.started.elapsed(),
                seed: ANIMATION_SEED,
                fps: self.animation_fps,
            },
        );
    }
}

/// How long a digit takes to flip when the animation is on.
const DEFAULT_FLIP_MS: u64 = 180;
/// Frames per second of the background animation.
const DEFAULT_ANIMATION_FPS: u32 = 12;
/// Seed for the random part of the animations, fixed so a frame is reproducible.
const ANIMATION_SEED: u64 = 0x5eed_1234;
/// The widget row never takes more than this from the clock when ill is on.
const WIDGET_ROW_MIN_HEIGHT: u16 = 8;
/// How long a shortcut confirmation stays on screen.
const STATUS_LINGER: std::time::Duration = std::time::Duration::from_millis(1600);

/// The mode to start in. A name the app does not know falls back to the clock
/// instead of refusing to start.
fn normalize_mode(name: &str) -> String {
    let name = name.trim().to_ascii_lowercase();
    match name.as_str() {
        "clock" | "timer" | "stopwatch" | "countdown" => name,
        _ => "clock".to_string(),
    }
}

/// The clock's time format, when the config names a valid one. An invalid
/// strftime is refused here instead of panicking while drawing.
fn clock_format(config: &ClockConfig) -> Option<String> {
    named(config.format.as_deref()).and_then(|format| {
        if modes::valid_strftime(&format) {
            Some(format)
        } else {
            eprintln!("aviso: formato de hora inválido: {format}; usando o padrão");
            None
        }
    })
}

/// The date format, when the config names a valid one.
fn date_format(config: &ClockConfig) -> Option<String> {
    named(config.date_format.as_deref()).and_then(|format| {
        if modes::valid_strftime(&format) {
            Some(format)
        } else {
            eprintln!("aviso: formato de data inválido: {format}; usando o padrão");
            None
        }
    })
}

/// The extra time zones, skipping names chrono-tz does not know.
fn world_zones(config: &ClockConfig) -> Vec<Tz> {
    config
        .world
        .iter()
        .filter_map(|name| match name.trim().parse::<Tz>() {
            Ok(zone) => Some(zone),
            Err(_) => {
                eprintln!("aviso: fuso desconhecido em world: {name}");
                None
            }
        })
        .collect()
}

/// When the config file was last written, used to notice edits.
fn config_stamp(path: &PathBuf) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// The starting digit face: the flag wins, then the config, then bricks.
fn font_from_config(flag: Option<FontStyle>, config: &ClockConfig) -> FontStyle {
    if let Some(font) = flag {
        return font;
    }
    let Some(name) = named(config.font.as_deref()) else {
        return FontStyle::Bricks;
    };

    FontStyle::from_name(&name).unwrap_or_else(|| {
        eprintln!(
            "aviso: estilo de fonte desconhecido: {name}; use um de: {}",
            FontStyle::list()
        );
        FontStyle::Bricks
    })
}

/// The starting background animation: the flag wins, then the config, then none.
fn animation_from_config(flag: Option<Animation>, config: &ClockConfig) -> Animation {
    if let Some(animation) = flag {
        return animation;
    }
    let Some(name) = named(config.animation.as_deref()) else {
        return Animation::None;
    };

    Animation::from_name(&name).unwrap_or_else(|| {
        eprintln!(
            "aviso: animação desconhecida: {name}; use uma de: {}",
            Animation::list()
        );
        Animation::None
    })
}

/// A trimmed, non-empty name taken from the config.
fn named(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

fn parse_font(input: &str) -> Result<FontStyle, String> {
    FontStyle::from_name(input).ok_or_else(|| {
        format!(
            "estilo de fonte desconhecido: {input}; use um de: {}",
            FontStyle::list()
        )
    })
}

fn parse_animation(input: &str) -> Result<Animation, String> {
    Animation::from_name(input).ok_or_else(|| {
        format!(
            "animação desconhecida: {input}; use uma de: {}",
            Animation::list()
        )
    })
}

fn configured_durations(configured: &[String]) -> Vec<ChronoDuration> {
    let durations: Vec<ChronoDuration> = configured
        .iter()
        .filter_map(|duration| parse_duration(duration).ok())
        .collect();

    if durations.is_empty() {
        vec![ChronoDuration::minutes(5)]
    } else {
        durations
    }
}

fn parse_duration(input: &str) -> Result<ChronoDuration, String> {
    let input = input.trim();
    let Some(suffix) = input.chars().last() else {
        return Err("duração vazia".to_string());
    };
    let number = &input[..input.len() - suffix.len_utf8()];
    let value: i64 = number
        .parse()
        .map_err(|_| format!("{input} não é uma duração válida"))?;

    let duration = match suffix.to_ascii_lowercase() {
        's' => ChronoDuration::try_seconds(value),
        'm' => ChronoDuration::try_minutes(value),
        'h' => ChronoDuration::try_hours(value),
        'd' => ChronoDuration::try_days(value),
        _ => return Err(format!("unidade desconhecida em {input}")),
    };

    duration.ok_or_else(|| format!("duração grande demais: {input}"))
}

fn parse_scale(input: &str) -> Result<u16, String> {
    let scale: u16 = input
        .trim()
        .parse()
        .map_err(|_| format!("escala inválida: {input}"))?;
    if (1..=crate::size_input::MAX_SCALE).contains(&scale) {
        Ok(scale)
    } else {
        Err(format!(
            "a escala precisa ficar entre 1 e {}",
            crate::size_input::MAX_SCALE
        ))
    }
}

fn parse_theme(input: &str) -> Result<String, String> {
    let theme = input.trim();
    if theme.is_empty() {
        Err("o tema não pode ficar vazio".to_string())
    } else {
        Ok(theme.to_string())
    }
}

fn parse_timezone(input: &str) -> Result<Tz, String> {
    input.parse::<Tz>().map_err(|error| error.to_string())
}

fn local_datetime(date_time: NaiveDateTime) -> Result<DateTime<Local>, String> {
    match Local.from_local_datetime(&date_time) {
        LocalResult::Single(date_time) => Ok(date_time),
        LocalResult::Ambiguous(_, _) => Err(format!("horário ambíguo: {date_time}")),
        LocalResult::None => Err(format!("horário inválido: {date_time}")),
    }
}

fn parse_datetime(input: &str) -> Result<DateTime<Local>, String> {
    let input = input.trim();
    let today = Local::now().date_naive();

    for format in ["%H:%M", "%H:%M:%S"] {
        if let Ok(time) = NaiveTime::parse_from_str(input, format) {
            return local_datetime(NaiveDateTime::new(today, time));
        }
    }

    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        let midnight = NaiveTime::from_hms_opt(0, 0, 0).expect("meia-noite é válida");
        return local_datetime(NaiveDateTime::new(date, midnight));
    }

    if let Ok(date_time) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S") {
        return local_datetime(date_time);
    }

    if let Ok(date_time) = DateTime::parse_from_rfc3339(input) {
        return Ok(date_time.with_timezone(&Local));
    }

    Err(format!("formato de data/hora não reconhecido: {input}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use crossterm::event::{KeyEvent, KeyModifiers};

    /// Config inexistente, para o teste não depender do arquivo da máquina.
    const NO_CONFIG: &str = "config-inexistente-do-teste.toml";

    /// Builds the app from arguments plus a config path that does not exist,
    /// unless the test brings its own.
    fn app_from(args: &[&str]) -> App {
        let mut argv = vec!["my-dev-clock"];
        if !args.contains(&"--config") {
            argv.extend(["--config", NO_CONFIG]);
        }
        argv.extend(args.iter().copied());
        App::from_cli(Cli::try_parse_from(argv).expect("linha de comando válida"))
    }

    fn write_config(name: &str, contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("my-dev-clock-app-{name}.toml"));
        std::fs::write(&path, contents).expect("grava a config do teste");
        path
    }

    #[test]
    fn the_command_line_definition_passes_clap_self_checks() {
        // Catches duplicate short flags between the global display options and
        // the subcommands, which clap only verifies in debug builds.
        Cli::command().debug_assert();
    }

    #[test]
    fn durations_accept_the_documented_units() {
        assert_eq!(parse_duration("10s").unwrap(), ChronoDuration::seconds(10));
        assert_eq!(parse_duration("5M").unwrap(), ChronoDuration::minutes(5));
        assert_eq!(parse_duration("2h").unwrap(), ChronoDuration::hours(2));
        assert_eq!(parse_duration("1d").unwrap(), ChronoDuration::days(1));
        assert!(parse_duration("10").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("").is_err());
        assert!(parse_duration("999999999999999999d").is_err());
    }

    #[test]
    fn the_scale_is_limited_to_the_supported_range() {
        assert_eq!(parse_scale("1").unwrap(), 1);
        assert_eq!(parse_scale("12").unwrap(), 12);
        assert!(parse_scale("0").is_err());
        assert!(parse_scale("13").is_err());
        assert!(parse_scale("x").is_err());
    }

    #[test]
    fn datetimes_accept_the_documented_shapes() {
        assert!(parse_datetime("20:00").is_ok());
        assert!(parse_datetime("20:00:30").is_ok());
        assert!(parse_datetime("2026-01-01").is_ok());
        assert!(parse_datetime("2026-12-25 20:00:00").is_ok());
        assert!(parse_datetime("2026-12-25T20:00:00-03:00").is_ok());
        assert!(parse_datetime("nada disso").is_err());
    }

    #[test]
    fn display_flags_work_before_and_after_the_mode_name() {
        let cli = Cli::try_parse_from(["my-dev-clock", "stopwatch", "--no-seconds", "-T", "Foco"])
            .unwrap();
        assert!(matches!(cli.mode, Some(Mode::Stopwatch)));
        assert_eq!(cli.display.show_seconds(), Some(false));
        assert_eq!(cli.display.titles, vec!["Foco"]);

        let cli = Cli::try_parse_from(["my-dev-clock", "--no-millis", "-P", "timer", "-d", "10m"])
            .unwrap();
        assert!(matches!(cli.mode, Some(Mode::Timer { .. })));
        assert_eq!(cli.display.show_millis(), Some(false));
        assert_eq!(cli.display.start_paused(), Some(true));
    }

    #[test]
    fn a_later_flag_wins_over_its_opposite() {
        let cli = Cli::try_parse_from(["my-dev-clock", "--millis", "--no-millis"]).unwrap();
        assert_eq!(cli.display.show_millis(), Some(false));

        let cli = Cli::try_parse_from(["my-dev-clock"]).unwrap();
        assert_eq!(cli.display.show_seconds(), None);
        assert_eq!(cli.display.show_millis(), None);
        assert_eq!(cli.display.show_date(), None);
        assert_eq!(cli.display.start_paused(), None);
    }

    fn display_config(
        title: Option<&str>,
        seconds: Option<bool>,
        millis: Option<bool>,
    ) -> DisplayConfig {
        DisplayConfig {
            title: title.map(str::to_string),
            show_date: None,
            show_seconds: seconds,
            show_millis: millis,
            start_paused: None,
        }
    }

    #[test]
    fn precedence_is_cli_then_mode_then_default_then_builtin() {
        let mode = display_config(Some("Modo"), Some(false), None);
        let default = display_config(Some("Padrão"), Some(true), Some(false));

        let resolved = DisplayOptions::resolve(
            &DisplayArgs::default(),
            None,
            None,
            STOPWATCH_DEFAULTS,
            Vec::new(),
        );
        assert!(resolved.show_seconds && resolved.show_millis && !resolved.start_paused);

        let resolved = DisplayOptions::resolve(
            &DisplayArgs::default(),
            Some(&mode),
            Some(&default),
            STOPWATCH_DEFAULTS,
            Vec::new(),
        );
        assert_eq!(resolved.titles, vec!["Modo"]);
        assert!(!resolved.show_seconds);
        assert!(!resolved.show_millis);

        let args = DisplayArgs {
            seconds: true,
            titles: vec!["CLI".to_string()],
            ..DisplayArgs::default()
        };
        let resolved = DisplayOptions::resolve(
            &args,
            Some(&mode),
            Some(&default),
            STOPWATCH_DEFAULTS,
            Vec::new(),
        );
        assert_eq!(resolved.titles, vec!["CLI"]);
        assert!(resolved.show_seconds);

        let resolved = DisplayOptions::resolve(
            &DisplayArgs::default(),
            Some(&mode),
            Some(&default),
            TIMER_DEFAULTS,
            vec!["Foco".to_string(), "Pausa".to_string()],
        );
        assert_eq!(resolved.titles, vec!["Foco", "Pausa"]);
        assert_eq!(resolved.title().as_deref(), Some("Foco"));
    }

    #[test]
    fn configured_durations_fall_back_when_every_value_is_invalid() {
        assert_eq!(
            configured_durations(&["nada".to_string()]),
            vec![ChronoDuration::minutes(5)]
        );
        assert_eq!(configured_durations(&[]), vec![ChronoDuration::minutes(5)]);
        assert_eq!(
            configured_durations(&["25m".to_string(), "5m".to_string()]),
            vec![ChronoDuration::minutes(25), ChronoDuration::minutes(5)]
        );
    }

    #[test]
    fn the_timer_starts_from_the_command_line_durations() {
        let app = app_from(&["timer", "-d", "10m"]);
        assert_eq!(app.mode, "timer");
        let timer = app.timer.expect("o timer foi construído");
        assert_eq!(timer.durations, vec![ChronoDuration::minutes(10)]);
    }

    #[test]
    fn an_unknown_configured_mode_falls_back_to_the_clock() {
        assert_eq!(normalize_mode("stopwatch"), "stopwatch");
        assert_eq!(normalize_mode("  Timer "), "timer");
        assert_eq!(normalize_mode("banana"), "clock");
        assert_eq!(normalize_mode(""), "clock");
    }

    #[test]
    fn the_mode_can_come_from_the_config_file() {
        let path = write_config("modo", "[default]\nmode = \"stopwatch\"\n");
        let cli = Cli::try_parse_from([
            "my-dev-clock",
            "--config",
            path.to_str().expect("caminho utf-8"),
        ])
        .expect("linha de comando válida");
        let app = App::from_cli(cli);

        assert_eq!(app.mode, "stopwatch");
        assert!(app.stopwatch.is_some());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn typing_a_scale_resizes_the_clock_and_escape_leaves_it_alone() {
        let mut app = app_from(&[]);
        assert_eq!(app.scale, None);

        app.on_key(KeyCode::Char('s'));
        assert!(app.scale_input.is_some());

        app.on_key(KeyCode::Char('5'));
        assert_eq!(app.scale, Some(5), "pré-visualização ao digitar");

        app.on_key(KeyCode::Enter);
        assert!(app.scale_input.is_none());
        assert_eq!(app.scale, Some(5));
    }

    #[test]
    fn while_typing_a_scale_the_other_shortcuts_are_swallowed() {
        let mut app = app_from(&[]);
        app.on_key(KeyCode::Char('s'));

        assert_eq!(app.on_key(KeyCode::Char('q')), Action::None);
        assert_eq!(app.mode, "clock");
        assert!(app.scale_input.is_some());

        app.on_key(KeyCode::Esc);
        assert_eq!(app.on_key(KeyCode::Char('q')), Action::Quit);
    }

    #[test]
    fn shortcut_keys_switch_modes_pause_and_cycle_themes() {
        let mut app = app_from(&[]);

        app.on_key(KeyCode::Char('t'));
        assert_eq!(app.mode, "timer");

        app.on_key(KeyCode::Char('w'));
        assert_eq!(app.mode, "stopwatch");
        assert!(!app.stopwatch.as_ref().unwrap().is_paused());
        app.on_key(KeyCode::Char(' '));
        assert!(app.stopwatch.as_ref().unwrap().is_paused());

        app.on_key(KeyCode::Char('c'));
        assert_eq!(app.mode, "clock");

        let first = app.theme_name().to_string();
        app.on_key(KeyCode::Char('T'));
        assert_ne!(app.theme_name(), first);
    }

    #[test]
    fn a_lowercase_t_still_switches_to_the_timer() {
        let mut app = app_from(&[]);
        let theme = app.theme_name().to_string();

        app.on_key(KeyCode::Char('t'));
        assert_eq!(app.mode, "timer");
        assert_eq!(app.theme_name(), theme);
    }

    #[test]
    fn z_toggles_the_widgets() {
        let mut app = app_from(&[]);

        app.on_key(KeyCode::Char('z'));
        assert!(app.widgets.hidden());
        app.on_key(KeyCode::Char('z'));
        assert!(!app.widgets.hidden());
    }

    #[test]
    fn the_app_can_start_in_every_mode_from_the_command_line() {
        for args in [
            vec!["my-dev-clock", "clock"],
            vec!["my-dev-clock", "timer"],
            vec!["my-dev-clock", "stopwatch"],
            vec!["my-dev-clock", "countdown", "-t", "20:00"],
        ] {
            let app = app_from(&args[1..]);
            assert!(
                app.clock.is_some()
                    || app.timer.is_some()
                    || app.stopwatch.is_some()
                    || app.countdown.is_some(),
                "{args:?}"
            );
        }
    }

    #[test]
    fn a_countdown_without_any_target_is_not_built() {
        let app = app_from(&["countdown", "-t", "20:00"]);
        assert!(app.countdown.is_some());

        let app = app_from(&[]);
        assert!(app.countdown.is_none());
    }
    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new_with_kind(
            code,
            KeyModifiers::NONE,
            KeyEventKind::Press,
        ))
    }

    fn release(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new_with_kind(
            code,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ))
    }

    #[test]
    fn a_key_release_is_not_a_second_shortcut() {
        // No Windows apertar e soltar chegam os dois como eventos, então tratar
        // os dois aplica cada atalho duas vezes.
        let mut app = app_from(&[]);

        let font = app.font;
        app.on_event(press(KeyCode::Char('F')));
        let after_press = app.font;
        assert_ne!(after_press, font, "o F deveria trocar a fonte");

        app.on_event(release(KeyCode::Char('F')));
        assert_eq!(app.font, after_press, "soltar o F não troca de novo");

        let fill = app.fill;
        app.on_event(press(KeyCode::Char('L')));
        let toggled = app.fill;
        assert_ne!(toggled, fill, "o L deveria alternar a tela cheia");
        app.on_event(release(KeyCode::Char('L')));
        assert_eq!(app.fill, toggled, "soltar o L não alterna de volta");
    }

    #[test]
    fn a_single_press_cycles_the_font_through_every_face() {
        let mut app = app_from(&[]);
        let mut seen = vec![app.font];

        for _ in 1..FONTS.len() {
            app.on_event(press(KeyCode::Char('F')));
            assert!(
                !seen.contains(&app.font),
                "a face {} apareceu duas vezes antes da hora",
                app.font.name()
            );
            seen.push(app.font);
        }
        assert_eq!(seen.len(), FONTS.len());

        app.on_event(press(KeyCode::Char('F')));
        assert_eq!(
            app.font, seen[0],
            "depois da última volta, volta à primeira"
        );
    }

    #[test]
    fn a_single_press_cycles_the_animation_through_every_kind() {
        let mut app = app_from(&[]);
        let mut seen = vec![app.animation];

        for _ in 1..ANIMATIONS.len() {
            app.on_event(press(KeyCode::Char('A')));
            assert!(
                !seen.contains(&app.animation),
                "a animação {} apareceu duas vezes antes da hora",
                app.animation.name()
            );
            seen.push(app.animation);
        }
        assert_eq!(seen.len(), ANIMATIONS.len());

        app.on_event(press(KeyCode::Char('A')));
        assert_eq!(app.animation, seen[0]);
    }

    #[test]
    fn a_single_press_toggles_the_full_screen_layout() {
        let mut app = app_from(&[]);
        let start = app.fill;

        app.on_event(press(KeyCode::Char('L')));
        assert_ne!(app.fill, start);
        app.on_event(press(KeyCode::Char('L')));
        assert_eq!(app.fill, start);
    }

    #[test]
    fn every_shortcut_has_an_effect_on_a_single_press() {
        // O mapa dos atalhos que o README promete, cada um aplicado uma vez.
        let mut app = app_from(&[]);

        let theme = app.theme_name().to_string();
        app.on_event(press(KeyCode::Char('T')));
        assert_ne!(app.theme_name(), theme, "T troca o tema");

        let font = app.font;
        app.on_event(press(KeyCode::Char('F')));
        assert_ne!(app.font, font, "F troca a fonte");

        let animation = app.animation;
        app.on_event(press(KeyCode::Char('A')));
        assert_ne!(app.animation, animation, "A troca a animação");

        let fill = app.fill;
        app.on_event(press(KeyCode::Char('L')));
        assert_ne!(app.fill, fill, "L alterna a tela cheia");

        let hidden = app.widgets.hidden();
        app.on_event(press(KeyCode::Char('z')));
        assert_ne!(app.widgets.hidden(), hidden, "z esconde os widgets");

        app.on_event(press(KeyCode::Char('s')));
        assert!(app.scale_input.is_some(), "s abre o campo de tamanho");
        app.on_event(press(KeyCode::Esc));
        assert!(app.scale_input.is_none(), "Esc fecha o campo de tamanho");

        app.on_event(press(KeyCode::Char('t')));
        assert_eq!(app.mode, "timer", "t vai para o timer");
        assert!(
            app.timer.as_ref().is_some_and(|timer| !timer.is_paused()),
            "o timer nasce rodando"
        );
        app.on_event(press(KeyCode::Char(' ')));
        assert!(
            app.timer.as_ref().is_some_and(Pause::is_paused),
            "espaço pausa"
        );

        app.on_event(press(KeyCode::Char('w')));
        assert_eq!(app.mode, "stopwatch", "w vai para o cronômetro");
        app.on_event(press(KeyCode::Char('c')));
        assert_eq!(app.mode, "clock", "c volta para o relógio");

        // As teclas de rolagem não podem quebrar nada, mesmo sem widgets.
        for key in [
            KeyCode::Home,
            KeyCode::End,
            KeyCode::PageUp,
            KeyCode::PageDown,
        ] {
            assert_eq!(app.on_event(press(key)), Action::None);
        }
    }

    #[test]
    fn the_quit_key_only_fires_on_the_press() {
        let mut app = app_from(&[]);
        assert_eq!(app.on_event(press(KeyCode::Char('q'))), Action::Quit);
        assert_eq!(app.on_event(release(KeyCode::Char('q'))), Action::None);
    }

    #[test]
    fn the_stopwatch_lap_is_announced_and_listed() {
        let mut app = app_from(&["stopwatch"]);

        app.on_event(press(KeyCode::Char('l')));
        app.on_event(press(KeyCode::Char('l')));

        let stopwatch = app.stopwatch.as_ref().expect("cronômetro");
        assert_eq!(stopwatch.laps().len(), 2);

        let announcement = app
            .status
            .as_ref()
            .map(|status| status.text.clone())
            .unwrap_or_default();
        assert!(announcement.starts_with("volta 2"), "{announcement}");

        let csv = stopwatch.laps_csv();
        assert!(csv.starts_with("volta,volta_s,total_s\n"), "{csv}");
        assert_eq!(csv.lines().count(), 3, "{csv}");
    }

    #[test]
    fn the_timer_controls_move_in_time() {
        let mut app = app_from(&["timer", "-d", "10m"]);

        app.on_event(press(KeyCode::Char('+')));
        assert_eq!(
            app.timer.as_ref().expect("timer").durations[0],
            ChronoDuration::minutes(11)
        );

        app.on_event(press(KeyCode::Char('-')));
        app.on_event(press(KeyCode::Char('-')));
        assert_eq!(
            app.timer.as_ref().expect("timer").durations[0],
            ChronoDuration::minutes(9)
        );

        // Pular leva direto ao fim do intervalo; com uma duração só, acaba.
        app.on_event(press(KeyCode::Char('n')));
        let (remaining, _) = app.timer.as_ref().expect("timer").remaining();
        assert!(remaining <= ChronoDuration::zero(), "{remaining}");

        app.on_event(press(KeyCode::Char('r')));
        let (remaining, _) = app.timer.as_ref().expect("timer").remaining();
        assert!(remaining > ChronoDuration::zero(), "{remaining}");
    }

    #[test]
    fn the_timer_controls_do_nothing_in_another_mode() {
        let mut app = app_from(&["stopwatch"]);
        // Nenhuma dessas teclas pode quebrar quando não há timer.
        for key in ["n", "+", "-", "r", "l"] {
            app.on_event(press(KeyCode::Char(key.chars().next().expect("tecla"))));
        }
        assert_eq!(app.mode, "stopwatch");
    }

    #[test]
    fn the_help_overlay_opens_and_closes() {
        let mut app = app_from(&[]);
        assert!(!app.help);

        app.on_event(press(KeyCode::Char('?')));
        assert!(app.help, "? abre a ajuda");
        app.on_event(press(KeyCode::Char('?')));
        assert!(!app.help, "? fecha a ajuda");

        app.on_event(press(KeyCode::Char('?')));
        app.on_event(press(KeyCode::Esc));
        assert!(!app.help, "Esc fecha a ajuda");
    }

    #[test]
    fn the_help_lists_every_shortcut_the_readme_promises() {
        let help = HELP_LINES.join("\n");
        for key in [
            "q",
            "espaço",
            "c / w / t",
            "Shift+T",
            "Shift+F",
            "Shift+A",
            "Shift+L",
            "z",
            "s",
            "?",
            "l",
            "n",
            "+ / -",
            "r",
            "Home / End",
            "PgUp / PgDn",
        ] {
            assert!(help.contains(key), "a ajuda não fala de {key}");
        }
    }

    #[test]
    fn the_custom_palette_in_the_config_is_applied() {
        let path = write_config(
            "tema",
            "[clock]\nthemes = [\"custom\"]\n\n[theme.custom]\nclock = \"#00ff88\"\n",
        );
        let app = app_from(&["--config", path.to_str().expect("utf-8")]);

        assert_eq!(app.theme_name(), "custom");
        assert_eq!(app.theme().clock.fg, Some(Color::Rgb(0, 255, 136)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn editing_the_config_file_reloads_it() {
        let path = write_config("recarga", "[clock]\nfont = \"bricks\"\n");
        let mut app = app_from(&["--config", path.to_str().expect("utf-8")]);
        assert_eq!(app.font, FontStyle::Bricks);

        // O carimbo de tempo precisa mudar, então a escrita vem depois.
        std::thread::sleep(std::time::Duration::from_millis(30));
        std::fs::write(&path, "[clock]\nfont = \"slim\"\n").expect("reescreve a config");

        app.reload_config_if_changed();
        assert_eq!(app.font, FontStyle::Slim);

        let announcement = app
            .status
            .as_ref()
            .map(|status| status.text.clone())
            .unwrap_or_default();
        assert!(announcement.contains("recarregada"), "{announcement}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn print_fills_the_known_placeholders_and_leaves_the_rest() {
        let app = app_from(&[]);
        let line = app.print_line("{time} · {host} · {nao existe}");

        assert!(!line.contains("{time}"), "{line}");
        assert!(!line.contains("{host}"), "{line}");
        assert!(
            line.contains("{nao existe}"),
            "um erro de digitação deve aparecer"
        );
    }

    #[test]
    fn the_look_shortcuts_work_even_while_typing_a_size() {
        let mut app = app_from(&[]);
        app.on_event(press(KeyCode::Char('s')));
        assert!(app.scale_input.is_some());

        let font = app.font;
        app.on_event(press(KeyCode::Char('F')));
        assert_ne!(app.font, font, "o F deveria funcionar com o campo aberto");
        assert!(app.scale_input.is_some(), "o campo continua aberto");

        app.on_event(press(KeyCode::Char('5')));
        assert_eq!(app.scale, Some(5));
        app.on_event(press(KeyCode::Enter));
        assert!(app.scale_input.is_none());
    }

    #[test]
    fn every_look_shortcut_announces_what_it_changed() {
        let mut app = app_from(&[]);

        for (key, expected) in [
            ('F', "fonte: "),
            ('A', "animação: "),
            ('L', "tela cheia: "),
            ('T', "tema: "),
        ] {
            app.on_event(press(KeyCode::Char(key)));
            let text = app
                .status
                .as_ref()
                .map(|status| status.text.clone())
                .unwrap_or_default();
            assert!(
                text.starts_with(expected),
                "{key} deveria anunciar {expected}, anunciou {text:?}"
            );
        }
    }

    #[test]
    fn the_full_screen_shortcut_says_when_it_changes_nothing() {
        // Sem widgets na tela o relógio já ocupa tudo, então a confirmação
        // precisa dizer isso em vez de deixar a impressão de que não funcionou.
        let mut app = app_from(&[]);
        app.on_event(press(KeyCode::Char('L')));

        let text = app
            .status
            .as_ref()
            .map(|status| status.text.clone())
            .unwrap_or_default();
        assert!(text.contains("sem widgets na tela"), "{text}");
    }
}
