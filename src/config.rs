use std::path::{Path, PathBuf};

use chrono_tz::Tz;
use serde::{Deserialize, Deserializer};

use crate::themes::BUILTIN;

pub const APP_DIR: &str = "my-dev-clock";
pub const CONFIG_FILE: &str = "config.toml";
pub const DEFAULT_WIDGET_REFRESH_SECS: u64 = 15 * 60;
pub const DEFAULT_WIDGET_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub default: DefaultConfig,
    #[serde(default)]
    pub clock: ClockConfig,
    #[serde(default)]
    pub timer: TimerConfig,
    #[serde(default)]
    pub stopwatch: StopwatchConfig,
    #[serde(default)]
    pub countdown: CountdownConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
}

/// A palette that replaces the built-in ones when the active theme is named
/// "custom". Every color is independent, and whatever is missing falls back to
/// the default palette for the base color.
#[derive(Debug, Default, Deserialize)]
pub struct ThemeConfig {
    #[serde(default)]
    pub custom: Option<CustomTheme>,
}

#[derive(Debug, Default, Deserialize)]
pub struct CustomTheme {
    #[serde(default)]
    pub clock: Option<String>,
    #[serde(default)]
    pub header: Option<String>,
    #[serde(default)]
    pub widget: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub marker: Option<String>,
    #[serde(default)]
    pub accent: Option<String>,
    #[serde(default)]
    pub animation: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DefaultConfig {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_color")]
    pub color: String,
    /// Starting digit scale. When absent the clock fits the terminal, and
    /// typing a size at runtime sets it explicitly.
    #[serde(default)]
    pub size: Option<u16>,
    #[serde(flatten)]
    pub display: DisplayConfig,
}

impl Default for DefaultConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            color: default_color(),
            size: None,
            display: DisplayConfig::default(),
        }
    }
}

/// Display options understood by every mode. They may appear under `[default]`
/// and under any mode section; precedence is command line, then the mode
/// section, then `[default]`, then the built-in default of that mode.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct DisplayConfig {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub show_date: Option<bool>,
    #[serde(default)]
    pub show_seconds: Option<bool>,
    #[serde(default)]
    pub show_millis: Option<bool>,
    #[serde(default)]
    pub start_paused: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ClockConfig {
    #[serde(flatten)]
    pub display: DisplayConfig,
    #[serde(default, deserialize_with = "deserialize_timezone")]
    pub timezone: Option<Tz>,
    /// Names cycled by `Shift+T`. Empty means the built-in list.
    #[serde(default)]
    pub themes: Vec<String>,
    /// Widest number of widget columns to use, capped by the terminal width.
    #[serde(default)]
    pub max_columns: Option<usize>,
    /// Digit face: bricks, slim, slant or flip.
    #[serde(default)]
    pub font: Option<String>,
    /// Flip a digit when it changes, the split-flap animation. When absent the
    /// face decides: lip turns it on, the others leave it off.
    #[serde(default)]
    pub flip: Option<bool>,
    /// How long a digit takes to flip, in milliseconds.
    #[serde(default)]
    pub flip_ms: Option<u64>,
    /// Background animation: none, matrix, plasma, waves or stars.
    #[serde(default)]
    pub animation: Option<String>,
    /// Frames per second the background animation is stepped at.
    #[serde(default)]
    pub animation_fps: Option<u32>,
    /// Let the clock fill its whole area instead of only the top half.
    #[serde(default)]
    pub fill: Option<bool>,
    /// strftime for the time in clock mode. Absent keeps what the display
    /// options imply; "%I:%M %p" gives a 12 hour clock.
    #[serde(default)]
    pub format: Option<String>,
    /// strftime for the date line. Absent keeps the localized form.
    #[serde(default)]
    pub date_format: Option<String>,
    /// Extra time zones shown next to the date, as IANA names.
    #[serde(default)]
    pub world: Vec<String>,
    #[serde(default)]
    pub widgets: Vec<WidgetConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WidgetConfig {
    /// `cpu`, `mem`, `weather`, `gh` or `command`.
    pub kind: String,
    /// Widget header. An empty string suppresses the header line entirely.
    #[serde(default)]
    pub title: Option<String>,
    /// Only for `kind = "command"`. A one-element list is run through the
    /// shell, a longer list is executed directly.
    #[serde(default, deserialize_with = "deserialize_widget_command")]
    pub command: Vec<String>,
    #[serde(default = "default_refresh")]
    pub refresh_secs: u64,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub weather: WeatherConfig,
    #[serde(default)]
    pub gh: GhConfig,
    #[serde(default)]
    pub git: GitConfig,
    #[serde(default)]
    pub http: HttpConfig,
    #[serde(default)]
    pub disk: DiskConfig,
    #[serde(default)]
    pub net: NetConfig,
    #[serde(default)]
    pub commits: CommitsConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GitConfig {
    /// Repository to inspect. Absent means the working directory.
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HttpConfig {
    #[serde(default)]
    pub url: Option<String>,
    /// Dot path into a JSON body, for example ".status" or ".data.count".
    #[serde(default)]
    pub path: Option<String>,
    /// Value the path is expected to hold; the widget then reports ok or not.
    #[serde(default)]
    pub expect: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DiskConfig {
    /// Mount point or drive to report. Absent picks the busiest disk.
    #[serde(default)]
    pub mount: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NetConfig {
    /// Interface to report. Absent sums every interface except loopback.
    #[serde(default)]
    pub interface: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WeatherConfig {
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lon: Option<f64>,
    /// `metric` (default) or `imperial`.
    #[serde(default)]
    pub units: Option<String>,
    /// Forecast rows to print, 1 to 7.
    #[serde(default)]
    pub days: Option<u8>,
}

/// The commits widget: your latest commits across every repository found under
/// the configured folders, so it answers "what have I been working on" without
/// caring which repository is open.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CommitsConfig {
    /// Folders scanned for repositories, up to `depth` levels down.
    #[serde(default)]
    pub roots: Vec<String>,
    /// Repositories given explicitly, for the ones outside any root.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Author to look for, as a name or an email. Absent asks git for the
    /// configured `user.email`.
    #[serde(default)]
    pub author: Option<String>,
    /// How many commits to show, five by default.
    #[serde(default)]
    pub limit: Option<usize>,
    /// How deep to look for repositories under each root, two by default.
    #[serde(default)]
    pub depth: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GhConfig {
    /// Maximum number of pending items listed.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Repository for the actions widget, as owner/name. Absent uses the
    /// repository of the current directory.
    #[serde(default)]
    pub repo: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TimerConfig {
    #[serde(default, deserialize_with = "deserialize_string_list")]
    pub durations: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_string_list")]
    pub titles: Vec<String>,
    #[serde(default)]
    pub repeat: bool,
    #[serde(default)]
    pub auto_quit: bool,
    #[serde(default, deserialize_with = "deserialize_string_list")]
    pub execute: Vec<String>,
    /// Ring the terminal bell when the time is up.
    #[serde(default)]
    pub bell: bool,
    #[serde(flatten)]
    pub display: DisplayConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct StopwatchConfig {
    #[serde(flatten)]
    pub display: DisplayConfig,
    /// Where to write the laps of a session when the app exits.
    #[serde(default)]
    pub laps_csv: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct CountdownConfig {
    #[serde(default)]
    pub time: Option<String>,
    /// Ring the terminal bell when the countdown reaches zero.
    #[serde(default)]
    pub bell: bool,
    #[serde(default)]
    pub continue_on_zero: bool,
    #[serde(default)]
    pub reverse: bool,
    #[serde(flatten)]
    pub display: DisplayConfig,
}

fn default_mode() -> String {
    "clock".to_string()
}

fn default_color() -> String {
    "green".to_string()
}

fn default_refresh() -> u64 {
    DEFAULT_WIDGET_REFRESH_SECS
}

fn default_timeout() -> u64 {
    DEFAULT_WIDGET_TIMEOUT_SECS
}

fn deserialize_timezone<'de, D>(deserializer: D) -> Result<Option<Tz>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<String>::deserialize(deserializer)? {
        Some(name) => name.parse().map(Some).map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

fn deserialize_widget_command<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum CommandValue {
        Program(String),
        Args(Vec<String>),
    }

    match CommandValue::deserialize(deserializer)? {
        CommandValue::Program(program) => Ok(vec![program]),
        CommandValue::Args(args) => Ok(args),
    }
}

fn deserialize_string_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
        One(String),
        Many(Vec<String>),
    }

    Ok(match Option::<Value>::deserialize(deserializer)? {
        Some(Value::One(value)) => vec![value],
        Some(Value::Many(values)) => values,
        None => Vec::new(),
    })
}

/// Where the config is looked for, highest priority first.
///
/// An absolute `$XDG_CONFIG_HOME` wins when it is set, then the OS-native
/// directory (`%APPDATA%\my-dev-clock` on Windows), and the Unix-style
/// `~/.config\my-dev-clock` last, so a config shared with a Unix machine is still
/// found when nothing else exists.
pub fn candidate_paths(explicit: Option<&Path>) -> Vec<PathBuf> {
    if let Some(path) = explicit {
        return vec![path.to_path_buf()];
    }

    let mut dirs = Vec::new();
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        dirs.push(xdg);
    }
    if let Some(native) = dirs::config_dir() {
        dirs.push(native);
    }
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".config"));
    }

    let mut paths = Vec::new();
    for dir in dirs {
        let path = dir.join(APP_DIR).join(CONFIG_FILE);
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

/// The path the app reads and writes: the first candidate that exists, or the
/// highest priority one when none do.
pub fn config_path(explicit: Option<&Path>) -> Option<PathBuf> {
    let paths = candidate_paths(explicit);
    paths
        .iter()
        .find(|path| path.exists())
        .cloned()
        .or_else(|| paths.into_iter().next())
}

/// Loads the config and reports where it came from. A missing file is not an
/// error; a malformed one is reported and then ignored.
pub fn load(explicit: Option<&Path>) -> (Config, Option<PathBuf>) {
    let Some(path) = candidate_paths(explicit)
        .into_iter()
        .find(|path| path.exists())
    else {
        return (Config::default(), None);
    };

    let Ok(content) = std::fs::read_to_string(&path) else {
        eprintln!("aviso: não consegui ler {}", path.display());
        return (Config::default(), None);
    };

    match toml::from_str::<Config>(&content) {
        Ok(config) => (config, Some(path)),
        Err(error) => {
            eprintln!(
                "erro ao interpretar {}: {error}\nusando a configuração padrão",
                path.display()
            );
            (Config::default(), Some(path))
        }
    }
}

/// Writes a commented starter config, creating the parent directory.
pub fn write_example(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, EXAMPLE)
}

pub const EXAMPLE: &str = include_str!("../config.example.toml");

/// The built-in theme list, used when the config names none.
pub fn default_themes() -> Vec<String> {
    BUILTIN.iter().map(|theme| (*theme).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_command_accepts_a_string_and_a_list() {
        let config: Config = toml::from_str(
            r#"
            [clock]
            [[clock.widgets]]
            kind = "command"
            command = "uptime"
            [[clock.widgets]]
            kind = "command"
            command = ["cmd", "/C", "echo oi"]
            "#,
        )
        .unwrap();

        assert_eq!(config.clock.widgets[0].command, vec!["uptime"]);
        assert_eq!(
            config.clock.widgets[1].command,
            vec!["cmd", "/C", "echo oi"]
        );
    }

    #[test]
    fn widget_defaults_are_applied() {
        let config: Config = toml::from_str(
            r#"
            [clock]
            [[clock.widgets]]
            kind = "cpu"
            "#,
        )
        .unwrap();

        let widget = &config.clock.widgets[0];
        assert_eq!(widget.kind, "cpu");
        assert_eq!(widget.refresh_secs, DEFAULT_WIDGET_REFRESH_SECS);
        assert_eq!(widget.timeout_secs, DEFAULT_WIDGET_TIMEOUT_SECS);
        assert!(widget.weather.city.is_none());
        assert!(widget.gh.limit.is_none());
    }

    #[test]
    fn weather_and_gh_settings_parse() {
        let config: Config = toml::from_str(
            r#"
            [clock]
            [[clock.widgets]]
            kind = "weather"
            [clock.widgets.weather]
            city = "Curitiba"
            units = "imperial"
            days = 5
            [[clock.widgets]]
            kind = "gh"
            [clock.widgets.gh]
            limit = 3
            "#,
        )
        .unwrap();

        let weather = &config.clock.widgets[0].weather;
        assert_eq!(weather.city.as_deref(), Some("Curitiba"));
        assert_eq!(weather.units.as_deref(), Some("imperial"));
        assert_eq!(weather.days, Some(5));
        assert_eq!(config.clock.widgets[1].gh.limit, Some(3));
    }

    #[test]
    fn the_look_of_the_clock_comes_from_the_config() {
        let config: Config = toml::from_str(
            r#"
            [clock]
            font = "flip"
            flip = true
            flip_ms = 240
            animation = "matrix"
            animation_fps = 20
            fill = true
            "#,
        )
        .unwrap();

        assert_eq!(config.clock.font.as_deref(), Some("flip"));
        assert_eq!(config.clock.flip, Some(true));
        assert_eq!(config.clock.flip_ms, Some(240));
        assert_eq!(config.clock.animation.as_deref(), Some("matrix"));
        assert_eq!(config.clock.animation_fps, Some(20));
        assert_eq!(config.clock.fill, Some(true));
    }

    #[test]
    fn the_custom_palette_and_the_new_clock_keys_parse() {
        let config: Config = toml::from_str(
            r##"
            [clock]
            format = "%I:%M %p"
            date_format = "%d/%m/%Y"
            world = ["America/New_York", "Europe/Lisbon"]

            [theme.custom]
            clock = "#00ff88"
            accent = "lightyellow"

            [stopwatch]
            laps_csv = "voltas.csv"

            [timer]
            bell = true

            [countdown]
            bell = true
            "##,
        )
        .unwrap();

        assert_eq!(config.clock.format.as_deref(), Some("%I:%M %p"));
        assert_eq!(config.clock.date_format.as_deref(), Some("%d/%m/%Y"));
        assert_eq!(
            config.clock.world,
            vec!["America/New_York", "Europe/Lisbon"]
        );

        let custom = config.theme.custom.expect("paleta custom");
        assert_eq!(custom.clock.as_deref(), Some("#00ff88"));
        assert_eq!(custom.accent.as_deref(), Some("lightyellow"));

        assert_eq!(config.stopwatch.laps_csv.as_deref(), Some("voltas.csv"));
        assert!(config.timer.bell);
        assert!(config.countdown.bell);
    }

    #[test]
    fn the_commits_widget_table_parses() {
        let config: Config = toml::from_str(
            r#"
            [clock]
            [[clock.widgets]]
            kind = "commits"

            [clock.widgets.commits]
            roots = ["C:/dev", "~/projetos"]
            paths = ["C:/avulso/repo"]
            author = "eu@exemplo.com"
            limit = 5
            depth = 3
            "#,
        )
        .unwrap();

        let commits = &config.clock.widgets[0].commits;
        assert_eq!(commits.roots, vec!["C:/dev", "~/projetos"]);
        assert_eq!(commits.paths, vec!["C:/avulso/repo"]);
        assert_eq!(commits.author.as_deref(), Some("eu@exemplo.com"));
        assert_eq!(commits.limit, Some(5));
        assert_eq!(commits.depth, Some(3));
    }

    #[test]
    fn the_new_widget_tables_parse() {
        let config: Config = toml::from_str(
            r#"
            [clock]
            [[clock.widgets]]
            kind = "git"
            [clock.widgets.git]
            path = "C:/dev/projeto"

            [[clock.widgets]]
            kind = "http"
            [clock.widgets.http]
            url = "http://localhost:3000/health"
            path = ".status"
            expect = "ok"

            [[clock.widgets]]
            kind = "disk"
            [clock.widgets.disk]
            mount = "C:"

            [[clock.widgets]]
            kind = "net"
            [clock.widgets.net]
            interface = "Ethernet"
            "#,
        )
        .unwrap();

        let widgets = &config.clock.widgets;
        assert_eq!(widgets[0].git.path.as_deref(), Some("C:/dev/projeto"));
        assert_eq!(
            widgets[1].http.url.as_deref(),
            Some("http://localhost:3000/health")
        );
        assert_eq!(widgets[1].http.path.as_deref(), Some(".status"));
        assert_eq!(widgets[1].http.expect.as_deref(), Some("ok"));
        assert_eq!(widgets[2].disk.mount.as_deref(), Some("C:"));
        assert_eq!(widgets[3].net.interface.as_deref(), Some("Ethernet"));
    }

    #[test]
    fn display_keys_parse_under_default_and_mode_sections() {
        let config: Config = toml::from_str(
            r##"
            [default]
            mode = "clock"
            color = "#00ff88"
            size = 3
            show_seconds = false

            [clock]
            timezone = "America/Sao_Paulo"
            themes = ["matrix", "neon"]

            [timer]
            durations = ["25m", "5m"]
            titles = ["Foco", "Pausa"]
            execute = "notify"

            [countdown]
            time = "20:00"
            "##,
        )
        .unwrap();

        assert_eq!(config.default.color, "#00ff88");
        assert_eq!(config.default.size, Some(3));
        assert_eq!(config.default.display.show_seconds, Some(false));
        assert_eq!(config.clock.timezone, Some(chrono_tz::America::Sao_Paulo));
        assert_eq!(config.clock.themes, vec!["matrix", "neon"]);
        assert_eq!(config.timer.durations, vec!["25m", "5m"]);
        assert_eq!(config.timer.titles, vec!["Foco", "Pausa"]);
        assert_eq!(config.timer.execute, vec!["notify"]);
        assert_eq!(config.countdown.time.as_deref(), Some("20:00"));
    }

    #[test]
    fn an_empty_config_leaves_everything_unset() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.default.mode, "clock");
        assert_eq!(config.default.color, "green");
        assert_eq!(config.default.display, DisplayConfig::default());
        assert!(config.clock.widgets.is_empty());
    }

    #[test]
    fn candidate_paths_put_an_explicit_path_first_and_alone() {
        let paths = candidate_paths(Some(Path::new(r"C:\tmp\meu.toml")));
        assert_eq!(paths, vec![PathBuf::from(r"C:\tmp\meu.toml")]);
    }

    #[test]
    fn candidate_paths_prefer_the_native_dir_over_dot_config() {
        let paths = candidate_paths(None);
        assert!(!paths.is_empty());

        let (Some(home), Some(native)) = (dirs::home_dir(), dirs::config_dir()) else {
            return;
        };
        let native = native.join(APP_DIR).join(CONFIG_FILE);
        let unix = home.join(".config").join(APP_DIR).join(CONFIG_FILE);

        // Em sistemas onde os dois caminhos coincidem não há ordem a conferir.
        if native == unix {
            return;
        }

        let native_at = paths
            .iter()
            .position(|path| *path == native)
            .expect("o diretório nativo deveria estar na lista");
        let unix_at = paths
            .iter()
            .position(|path| *path == unix)
            .expect("~/.config deveria continuar na lista");

        assert!(
            native_at < unix_at,
            "o diretório nativo precisa vir antes de ~/.config: {paths:?}"
        );
    }

    #[test]
    fn the_shipped_example_parses() {
        let config: Config = toml::from_str(EXAMPLE).unwrap();
        assert!(!config.clock.widgets.is_empty());
    }
}
