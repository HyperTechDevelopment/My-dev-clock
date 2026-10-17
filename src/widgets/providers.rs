use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, atomic::AtomicBool},
    time::{Duration, Instant},
};

use chrono::{DateTime, Datelike, FixedOffset, Local};
use serde::Deserialize;
use sysinfo::{Disks, Networks, System};

use crate::config::{
    CommitsConfig, DiskConfig, GhConfig, GitConfig, HttpConfig, NetConfig, WeatherConfig,
    WidgetConfig,
};
use crate::winproc;

/// Providers do their own HTTP with a timeout of their own, since the widget
/// engine only enforces a timeout on commands it launches.
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const GEOCODE_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";
const BAR_CELLS: usize = 12;
/// Where the repositories found under the roots are remembered, with the moment
/// they were found, so a refresh does not walk the disk again.
type RepoCache = Mutex<Option<(Instant, Vec<PathBuf>)>>;

/// Cap on how many repositories the commits widget inspects in one refresh.
const MAX_REPOS: usize = 40;

/// A trimmed, non-empty value taken from the config.
fn named(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

pub fn run_widget(
    config: &WidgetConfig,
    theme: &str,
    timeout: Duration,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) -> String {
    match config.kind.trim().to_ascii_lowercase().as_str() {
        "cpu" => cpu(),
        "mem" | "ram" => memory(),
        "disk" => disk(&config.disk),
        "net" | "network" => network(&config.net),
        "git" => git(&config.git, timeout, cancel, theme),
        "commits" | "log" => commits(&config.commits, timeout, cancel, theme),
        "http" | "health" => http(&config.http),
        "weather" | "clima" => weather(&config.weather),
        "gh" | "github" => github(&config.gh, timeout, cancel, theme),
        "actions" | "ci" => actions(&config.gh, timeout, cancel, theme),
        "command" | "comando" => winproc::run_capture(&config.command, timeout, cancel, theme),
        other => format!("[erro] tipo de widget desconhecido: {other}"),
    }
}

/// The processor load, averaged across the cores, and how many there are.
fn cpu_load() -> Option<(f32, usize)> {
    let mut system = System::new();
    // CPU usage is a difference between two samples, so the first refresh only
    // sets the baseline.
    system.refresh_cpu_usage();
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_cpu_usage();

    let cpus = system.cpus();
    if cpus.is_empty() {
        return None;
    }

    let usage = cpus.iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / cpus.len() as f32;
    Some((usage.clamp(0.0, 100.0), cpus.len()))
}

fn cpu() -> String {
    let Some((usage, cores)) = cpu_load() else {
        return "[erro] não consegui ler os núcleos".to_string();
    };

    format!("{}\n{usage:.1}%\n{cores} núcleos", bar(usage))
}

/// The processor load as one number, for one-line uses like `--print`.
pub fn cpu_line() -> String {
    match cpu_load() {
        Some((usage, _)) => format!("{usage:.0}%"),
        None => "?".to_string(),
    }
}

/// Memory used and total, and how much of it that is.
fn memory_load() -> Option<(f32, u64, u64)> {
    let mut system = System::new();
    system.refresh_memory();

    let total = system.total_memory();
    if total == 0 {
        return None;
    }
    let used = system.used_memory();

    Some((used as f32 * 100.0 / total as f32, used, total))
}

fn memory() -> String {
    let Some((percent, used, total)) = memory_load() else {
        return "[erro] não consegui ler a memória".to_string();
    };

    format!(
        "{}\n{percent:.1}%\n{} de {}",
        bar(percent),
        gigabytes(used),
        gigabytes(total)
    )
}

/// The memory use as one number, for one-line uses like `--print`.
pub fn memory_line() -> String {
    match memory_load() {
        Some((percent, _, _)) => format!("{percent:.0}%"),
        None => "?".to_string(),
    }
}

/// Bytes as a readable size, in the unit that fits.
fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn human_rate(bytes_per_second: f64) -> String {
    format!("{}/s", human_bytes(bytes_per_second.max(0.0) as u64))
}

/// The busiest disk, or the one the config names.
fn disk(config: &DiskConfig) -> String {
    let disks = Disks::new_with_refreshed_list();
    let wanted = config
        .mount
        .as_deref()
        .map(str::trim)
        .filter(|mount| !mount.is_empty())
        .map(str::to_ascii_lowercase);

    let chosen = match &wanted {
        Some(wanted) => disks.list().iter().find(|disk| {
            let mount = disk.mount_point().to_string_lossy().to_ascii_lowercase();
            mount.starts_with(wanted) || &mount == wanted
        }),
        // Without a mount point, the disk with the least free space is the one
        // worth watching.
        None => disks
            .list()
            .iter()
            .filter(|disk| disk.total_space() > 0)
            .min_by_key(|disk| disk.available_space()),
    };

    let Some(disk) = chosen else {
        return "[erro] não encontrei esse disco".to_string();
    };

    let total = disk.total_space();
    let free = disk.available_space();
    let used = total.saturating_sub(free);
    let percent = if total == 0 {
        0.0
    } else {
        used as f32 * 100.0 / total as f32
    };

    format!(
        "{}\n{percent:.1}% usado\n{} livres de {}",
        bar(percent),
        human_bytes(free),
        human_bytes(total)
    )
}

/// Network totals since boot, plus the rate since the previous refresh.
fn network(config: &NetConfig) -> String {
    let networks = Networks::new_with_refreshed_list();
    let wanted = config
        .interface
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_ascii_lowercase);

    let mut received = 0u64;
    let mut transmitted = 0u64;
    let mut interfaces = 0usize;

    for (name, data) in networks.list() {
        let lower = name.to_ascii_lowercase();
        if lower.contains("loopback") || lower.starts_with("lo") {
            continue;
        }
        if let Some(wanted) = &wanted
            && &lower != wanted
        {
            continue;
        }

        received += data.total_received();
        transmitted += data.total_transmitted();
        interfaces += 1;
    }

    if interfaces == 0 {
        return "[erro] não encontrei essa interface".to_string();
    }

    // A rate needs two readings, so the previous one is kept between refreshes.
    static PREVIOUS: OnceLock<Mutex<Option<(Instant, u64, u64)>>> = OnceLock::new();
    let now = Instant::now();
    let rate = PREVIOUS
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|mut slot| slot.replace((now, received, transmitted)))
        .and_then(|(then, was_received, was_transmitted)| {
            let seconds = now.saturating_duration_since(then).as_secs_f64();
            (seconds > 0.5).then(|| {
                (
                    received.saturating_sub(was_received) as f64 / seconds,
                    transmitted.saturating_sub(was_transmitted) as f64 / seconds,
                )
            })
        });

    let totals = format!(
        "{} recebidos · {} enviados",
        human_bytes(received),
        human_bytes(transmitted)
    );

    match rate {
        Some((down, up)) => format!("↓ {} · ↑ {}\n{totals}", human_rate(down), human_rate(up)),
        None => totals,
    }
}

/// The local repository: branch, how far it is from upstream and whether the
/// working tree has anything pending.
///
/// A single `git status` carries all of it, which is what keeps this widget
/// cheap enough to refresh every few seconds.
fn git(config: &GitConfig, timeout: Duration, cancel: Arc<AtomicBool>, theme: &str) -> String {
    let path = config
        .path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .unwrap_or(".");

    let command: Vec<String> = [
        "git",
        "-C",
        path,
        "status",
        "--porcelain=v2",
        "--branch",
        "--show-stash",
    ]
    .iter()
    .map(|arg| (*arg).to_string())
    .collect();

    let output = winproc::run_capture(&command, timeout, cancel, theme);
    if output.starts_with("[erro]") || output.starts_with("[timeout]") {
        return output;
    }

    parse_git_status(&output)
}

/// Turns `git status --porcelain=v2 --branch` into a compact block. Split out
/// so it can be tested against a captured status.
fn parse_git_status(output: &str) -> String {
    let mut branch = "(sem branch)".to_string();
    let mut ahead = 0u64;
    let mut behind = 0u64;
    let mut stash = 0u64;
    let mut changed = 0u64;

    for line in output.lines() {
        if let Some(value) = line.strip_prefix("# branch.head ") {
            branch = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("# branch.ab ") {
            for part in value.split_whitespace() {
                if let Some(count) = part.strip_prefix('+') {
                    ahead = count.parse().unwrap_or(0);
                } else if let Some(count) = part.strip_prefix('-') {
                    behind = count.parse().unwrap_or(0);
                }
            }
        } else if let Some(value) = line.strip_prefix("# stash ") {
            stash = value.trim().parse().unwrap_or(0);
        } else if !line.starts_with('#') && !line.trim().is_empty() {
            changed += 1;
        }
    }

    // A branch that was never pushed has no upstream line, which is not an
    // error: there is simply nothing to compare against.
    let mut marks = Vec::new();
    if changed > 0 {
        marks.push("*".to_string());
    }
    if ahead > 0 {
        marks.push(format!("↑{ahead}"));
    }
    if behind > 0 {
        marks.push(format!("↓{behind}"));
    }
    if stash > 0 {
        marks.push(format!("stash {stash}"));
    }

    let files = match changed {
        0 => "limpo".to_string(),
        1 => "1 arquivo pendente".to_string(),
        many => format!("{many} arquivos pendentes"),
    };

    if marks.is_empty() {
        format!("{branch}\n{files}")
    } else {
        format!("{branch} {}\n{files}", marks.join(" "))
    }
}

/// The branch and its state as a single token, for one-line uses like
/// --print.
pub fn git_branch_token(path: Option<&str>) -> String {
    let config = GitConfig {
        path: path.map(str::to_string),
    };
    let output = git(
        &config,
        Duration::from_secs(5),
        Arc::new(AtomicBool::new(false)),
        "default",
    );

    output.lines().next().unwrap_or("?").to_string()
}

/// One commit, already reduced to what the widget prints.
struct Commit {
    at: DateTime<FixedOffset>,
    project: String,
    subject: String,
}

/// The latest commits by the configured author, across every repository found
/// under the configured folders.
///
/// The question it answers is "what have I been working on", so it does not care
/// which repository the app happens to be running from.
fn commits(
    config: &CommitsConfig,
    timeout: Duration,
    cancel: Arc<AtomicBool>,
    theme: &str,
) -> String {
    let repos = discover_repos(config);
    if repos.is_empty() {
        return "[erro] configure 'roots' ou 'paths' no widget de commits".to_string();
    }

    let author = match named(config.author.as_deref()).or_else(git_identity) {
        Some(author) => author,
        None => "git".to_string(),
    };
    let limit = config.limit.unwrap_or(5).clamp(1, 20);

    let mut found: Vec<Commit> = Vec::new();
    for repo in &repos {
        let command: Vec<String> = [
            "git".to_string(),
            "-C".to_string(),
            repo.display().to_string(),
            "log".to_string(),
            // Case-insensitive, because the name in the config rarely matches the
            // case git stored.
            "-i".to_string(),
            format!("--author={author}"),
            "-n".to_string(),
            limit.to_string(),
            "--format=%h|%aI|%s".to_string(),
        ]
        .into_iter()
        .collect();

        let output = winproc::run_capture(&command, timeout, cancel.clone(), theme);
        // A repository with no commits, or none by this author, is not an error:
        // it simply contributes nothing.
        if output.starts_with("[erro]") || output.starts_with("[timeout]") {
            continue;
        }

        found.extend(output.lines().filter_map(|line| parse_commit(line, repo)));
    }

    found.sort_by_key(|commit| std::cmp::Reverse(commit.at));
    found.truncate(limit);

    if found.is_empty() {
        return format!("nenhum commit de {author} nos repositórios configurados");
    }

    found
        .iter()
        .map(|commit| {
            format!(
                "{} · {} · {}",
                commit.project,
                relative_age(commit.at),
                truncate(&commit.subject, 46)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `%h|%aI|%s` as git prints it.
fn parse_commit(line: &str, repo: &Path) -> Option<Commit> {
    let mut parts = line.splitn(3, '|');
    parts.next()?;
    let when = parts.next()?;
    let subject = parts.next()?;

    Some(Commit {
        at: DateTime::parse_from_rfc3339(when).ok()?,
        project: repo.file_name()?.to_string_lossy().to_string(),
        subject: subject.trim().to_string(),
    })
}

/// The repositories under the configured folders, remembered for a few minutes
/// so a refresh does not walk the disk again.
fn discover_repos(config: &CommitsConfig) -> Vec<PathBuf> {
    static CACHE: OnceLock<RepoCache> = OnceLock::new();
    const FRESH: Duration = Duration::from_secs(300);

    if let Some((when, repos)) = CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|cache| cache.clone())
        && when.elapsed() < FRESH
    {
        return repos;
    }

    let depth = config.depth.unwrap_or(2).clamp(1, 4);
    let mut repos: Vec<PathBuf> = config
        .paths
        .iter()
        .map(|path| expand_tilde(path))
        .filter(|path| path.join(".git").exists())
        .collect();

    for root in &config.roots {
        let root = expand_tilde(root);
        if root.is_dir() {
            walk_for_repos(&root, depth, &mut repos);
        }
    }

    repos.sort();
    repos.dedup();
    repos.truncate(MAX_REPOS);

    if let Ok(mut cache) = CACHE.get_or_init(|| Mutex::new(None)).lock() {
        *cache = Some((Instant::now(), repos.clone()));
    }
    repos
}

/// Looks for `.git` up to `depth` levels down. A repository rarely holds another
/// one, so the walk stops at the first hit.
fn walk_for_repos(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if out.len() >= MAX_REPOS {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join(".git").exists() {
            out.push(path);
            continue;
        }
        if depth > 1 {
            walk_for_repos(&path, depth - 1, out);
        }
    }
}

/// Expands a leading `~` so a path in the config reads naturally.
fn expand_tilde(path: &str) -> PathBuf {
    let path = path.trim();
    match path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        Some(rest) => dirs::home_dir()
            .map(|home| home.join(rest))
            .unwrap_or_else(|| PathBuf::from(path)),
        None => PathBuf::from(path),
    }
}

/// The email git is configured with, used when the config names no author.
fn git_identity() -> Option<String> {
    let command: Vec<String> = ["git", "config", "--get", "user.email"]
        .iter()
        .map(|arg| (*arg).to_string())
        .collect();
    let output = winproc::run_capture(
        &command,
        Duration::from_secs(5),
        Arc::new(AtomicBool::new(false)),
        "default",
    );

    let email = output.lines().next().unwrap_or_default().trim().to_string();
    (!email.is_empty() && !email.starts_with("[erro]") && !email.starts_with("[timeout]"))
        .then_some(email)
}

/// How long ago, in the shortest form that still says something.
fn relative_age(at: DateTime<FixedOffset>) -> String {
    let now = Local::now();
    let seconds = now
        .signed_duration_since(at.with_timezone(&Local))
        .num_seconds()
        .max(0);

    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    const MONTH: i64 = 30 * DAY;
    const YEAR: i64 = 365 * DAY;

    match seconds {
        s if s < MINUTE => "agora".to_string(),
        s if s < HOUR => format!("{} min", s / MINUTE),
        s if s < DAY => format!("{} h", s / HOUR),
        s if s < MONTH => format!("{} d", s / DAY),
        s if s < YEAR => plural(s / MONTH, "mês", "meses"),
        s => plural(s / YEAR, "ano", "anos"),
    }
}

/// One or many, without repeating the wording at every call site.
fn plural(count: i64, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {plural}")
    }
}

fn relative_age_iso(timestamp: &str) -> Option<String> {
    Some(relative_age(DateTime::parse_from_rfc3339(timestamp).ok()?))
}

/// A dev server, or any URL: the status line plus a value taken from the body.
fn http(config: &HttpConfig) -> String {
    let Some(url) = named(config.url.as_deref()) else {
        return "[erro] informe 'url' no widget de http".to_string();
    };

    let started = Instant::now();
    let (status, body) = match fetch(&url) {
        Ok(answer) => answer,
        Err(error) => return error,
    };
    let elapsed = started.elapsed().as_millis();

    let path = named(config.path.as_deref());
    let value = match path.as_deref() {
        Some(path) => json_path(&body, path).unwrap_or_else(|| format!("(sem {path})")),
        // Without a path, a short piece of the body is the useful part.
        None => body.chars().take(60).collect::<String>(),
    };

    let mut lines = vec![format!("{status} · {elapsed} ms")];

    if let Some(expect) = named(config.expect.as_deref()) {
        lines.push(if value.contains(&expect) {
            "esperado: ok".to_string()
        } else {
            format!("esperado: \"{expect}\"")
        });
    }

    // A linha do valor fica entre o status e o veredito.
    lines.insert(1, value);
    lines.join("\n")
}

/// One GET, returning the status and the body as text. A 4xx or 5xx is an
/// answer for a health check, not a failure, so it is not turned into an error.
fn fetch(url: &str) -> Result<(String, String), String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(HTTP_TIMEOUT))
        .http_status_as_error(false)
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let mut response = agent
        .get(url)
        .call()
        .map_err(|error| format!("[erro] {error}"))?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error| format!("[erro] resposta inesperada: {error}"))?;

    Ok((format!("HTTP {status}"), body))
}

/// Walks a dot path such as ".status" or ".data.items.0.name" into a JSON body.
fn json_path(body: &str, path: &str) -> Option<String> {
    let mut value: serde_json::Value = serde_json::from_str(body).ok()?;

    for key in path.trim_start_matches('.').split('.') {
        if key.is_empty() {
            continue;
        }
        value = match value {
            serde_json::Value::Object(map) => map.get(key).cloned()?,
            serde_json::Value::Array(items) => items.get(key.parse::<usize>().ok()?).cloned()?,
            _ => return None,
        };
    }

    Some(match value {
        serde_json::Value::String(text) => text,
        other => other.to_string(),
    })
}

/// The latest workflow runs, so a red pipeline shows up without a browser.
fn actions(config: &GhConfig, timeout: Duration, cancel: Arc<AtomicBool>, theme: &str) -> String {
    let limit = config.limit.unwrap_or(4).clamp(1, 10);

    let mut command = vec!["gh".to_string(), "run".to_string(), "list".to_string()];
    if let Some(repo) = named(config.repo.as_deref()) {
        command.push("--repo".to_string());
        command.push(repo);
    }
    command.push("--limit".to_string());
    command.push(limit.to_string());
    command.extend(
        ["--json", "workflowName,status,conclusion,headBranch"]
            .iter()
            .map(|arg| (*arg).to_string()),
    );

    let output = winproc::run_capture(&command, timeout, cancel, theme);
    if output.starts_with("[erro]") || output.starts_with("[timeout]") {
        return output;
    }

    match serde_json::from_str::<serde_json::Value>(&output) {
        Ok(serde_json::Value::Array(runs)) if runs.is_empty() => {
            "nenhuma execução recente".to_string()
        }
        Ok(serde_json::Value::Array(runs)) => {
            let lines: Vec<String> = runs
                .iter()
                .take(limit)
                .map(|run| {
                    let name = run
                        .get("workflowName")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("workflow");
                    let status = run
                        .get("status")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("");
                    let conclusion = run
                        .get("conclusion")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("");
                    let branch = run
                        .get("headBranch")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("");

                    let mark = match (status, conclusion) {
                        (_, "success") => "✓",
                        (_, "failure") => "✗",
                        ("in_progress", _) | ("queued", _) => "…",
                        (_, "cancelled") => "×",
                        _ => "·",
                    };
                    format!("{mark} {name} ({branch})")
                })
                .collect();
            lines.join("\n")
        }
        _ => "[erro] resposta inesperada do gh".to_string(),
    }
}

fn bar(percent: f32) -> String {
    let filled = ((percent.clamp(0.0, 100.0) / 100.0) * BAR_CELLS as f32).round() as usize;
    format!(
        "{}{}",
        "█".repeat(filled.min(BAR_CELLS)),
        "░".repeat(BAR_CELLS.saturating_sub(filled))
    )
}

fn gigabytes(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Units {
    Metric,
    Imperial,
}

impl Units {
    fn from_config(config: &WeatherConfig) -> Self {
        match config
            .units
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("imperial") => Self::Imperial,
            _ => Self::Metric,
        }
    }

    fn temperature(self) -> &'static str {
        match self {
            Self::Metric => "°C",
            Self::Imperial => "°F",
        }
    }

    fn wind(self) -> &'static str {
        match self {
            Self::Metric => "km/h",
            Self::Imperial => "mph",
        }
    }
}

fn weather(config: &WeatherConfig) -> String {
    let units = Units::from_config(config);
    let days = config.days.unwrap_or(2).clamp(1, 7) as usize;

    let (latitude, longitude) = match coordinates(config) {
        Ok(found) => found,
        // A failing lookup used to be reported as a missing city, which sent the
        // reader to the wrong place.
        Err(error) => return error,
    };

    let (temperature_unit, wind_unit) = match units {
        Units::Metric => ("celsius", "kmh"),
        Units::Imperial => ("fahrenheit", "mph"),
    };
    let url = format!(
        "{FORECAST_URL}?latitude={latitude}&longitude={longitude}\
         &current=temperature_2m,relative_humidity_2m,apparent_temperature,weather_code,wind_speed_10m\
         &daily=weather_code,temperature_2m_max,temperature_2m_min\
         &timezone=auto&forecast_days={days}\
         &temperature_unit={temperature_unit}&wind_speed_unit={wind_unit}"
    );

    let forecast: Forecast = match get_json(&url) {
        Ok(forecast) => forecast,
        Err(error) => return error,
    };

    let mut lines = Vec::new();
    let mut header = format!(
        "{:.1}{}",
        forecast.current.temperature_2m.unwrap_or_default(),
        units.temperature()
    );
    if let Some(code) = forecast.current.weather_code {
        header.push_str(" · ");
        header.push_str(description(code));
    }
    lines.push(header);

    let mut details = Vec::new();
    if let Some(apparent) = forecast.current.apparent_temperature {
        details.push(format!("sensação {apparent:.1}°"));
    }
    if let Some(humidity) = forecast.current.relative_humidity_2m {
        details.push(format!("umidade {humidity:.0}%"));
    }
    if let Some(wind) = forecast.current.wind_speed_10m {
        details.push(format!("vento {wind:.0} {}", units.wind()));
    }
    if !details.is_empty() {
        lines.push(details.join(" · "));
    }

    for index in 0..forecast.daily.time.len().min(days) {
        let label = day_label(&forecast.daily.time[index], index);
        let min = forecast
            .daily
            .temperature_2m_min
            .get(index)
            .copied()
            .flatten()
            .unwrap_or_default();
        let max = forecast
            .daily
            .temperature_2m_max
            .get(index)
            .copied()
            .flatten()
            .unwrap_or_default();
        lines.push(format!("{label:<6} {min:.1}° / {max:.1}°"));
    }

    lines.join("\n")
}

/// The geocoded coordinates of a city, remembered for the life of the process so
/// a refresh every few minutes costs a single request.
///
/// The error says which of the two problems happened: a missing location in the
/// config, or a lookup that did not answer.
fn coordinates(config: &WeatherConfig) -> Result<(f64, f64), String> {
    if let (Some(latitude), Some(longitude)) = (config.lat, config.lon) {
        return Ok((latitude, longitude));
    }

    let Some(city) = named(config.city.as_deref()) else {
        return Err("[erro] informe 'city' ou 'lat'/'lon' no widget de clima".to_string());
    };

    static CACHE: OnceLock<Mutex<HashMap<String, (f64, f64)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Some(found) = cache
        .lock()
        .ok()
        .and_then(|cache| cache.get(&city).copied())
    {
        return Ok(found);
    }

    let url = format!(
        "{GEOCODE_URL}?name={}&count=1&language=pt&format=json",
        urlencode(&city)
    );
    let response: GeocodeResponse = get_json(&url)?;
    let Some(first) = response.results.and_then(|list| list.into_iter().next()) else {
        return Err(format!("[erro] não encontrei a cidade: {city}"));
    };

    let found = (first.latitude, first.longitude);
    if let Ok(mut cache) = cache.lock() {
        cache.insert(city, found);
    }
    Ok(found)
}

fn day_label(date: &str, index: usize) -> String {
    match index {
        0 => "hoje".to_string(),
        1 => "amanhã".to_string(),
        _ => chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map(|date| {
                const WEEKDAYS: [&str; 7] = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];
                WEEKDAYS[date.weekday().num_days_from_sunday() as usize].to_string()
            })
            .unwrap_or_else(|_| date.to_string()),
    }
}

/// Percent-encoding for query values: city names carry spaces and accents.
fn urlencode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// What is pending for the authenticated user, in a single GraphQL call: the
/// three searches used to be three REST calls in a row.
fn github(config: &GhConfig, timeout: Duration, cancel: Arc<AtomicBool>, theme: &str) -> String {
    let limit = config.limit.unwrap_or(3).clamp(1, 10);
    let command = github_command(limit);

    let output = winproc::run_capture(&command, timeout, cancel, theme);
    if output.starts_with("[erro]") || output.starts_with("[timeout]") {
        return output;
    }

    match serde_json::from_str::<serde_json::Value>(&output) {
        Ok(payload) => format_pending(payload.get("data").unwrap_or(&payload), limit),
        Err(_) => "[erro] resposta inesperada do gh".to_string(),
    }
}

/// The one call the widget makes, kept apart so a test can check that it is one
/// call and that it asks for what the layout needs.
fn github_command(limit: usize) -> Vec<String> {
    // Duas listas de campos, porque `isDraft` e `reviewDecision` só existem em
    // PullRequest: pedir esses campos numa issue é erro de esquema.
    let fragment = |fields: &str| format!("issueCount nodes {{ {fields} }}");

    let query = format!(
        r#"query {{
  reviewRequested: search(query: "is:pr is:open review-requested:@me", type: ISSUE, first: {limit}) {{ {pr} }}
  authored: search(query: "is:pr is:open author:@me", type: ISSUE, first: {limit}) {{ {pr} }}
  assigned: search(query: "is:issue is:open assignee:@me", type: ISSUE, first: {limit}) {{ {issue} }}
}}"#,
        pr = fragment(
            " ... on PullRequest { number title isDraft reviewDecision updatedAt comments { totalCount } author { login } repository { nameWithOwner } }"
        ),
        issue = fragment(
            " ... on Issue { number title updatedAt comments { totalCount } author { login } repository { nameWithOwner } }"
        ),
    );

    vec![
        "gh".to_string(),
        "api".to_string(),
        "graphql".to_string(),
        "-F".to_string(),
        format!("query={query}"),
    ]
}

/// Which side of the item the reader is on, which decides what is worth
/// printing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Review,
    Authored,
    Issue,
}

/// Turns the answer into the block the widget shows: a line of counts, then one
/// section per category, with the items under their heading.
fn format_pending(data: &serde_json::Value, limit: usize) -> String {
    let sections = [
        ("reviewRequested", "revisão", "revisões", Role::Review),
        ("authored", "meu PR", "meus PRs", Role::Authored),
        ("assigned", "issue", "issues", Role::Issue),
    ];

    let mut counts = Vec::new();
    let mut body: Vec<String> = Vec::new();

    for (key, singular, plural, role) in sections {
        let node = data.get(key);
        let count = node
            .and_then(|node| node.get("issueCount"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        counts.push(if count == 1 {
            format!("1 {singular}")
        } else {
            format!("{count} {plural}")
        });

        let Some(items) = node
            .and_then(|node| node.get("nodes"))
            .and_then(serde_json::Value::as_array)
            .filter(|items| !items.is_empty())
        else {
            continue;
        };

        // O cabeçalho só aparece quando tem o que mostrar embaixo dele.
        body.push(plural.to_string());
        body.extend(items.iter().take(limit).map(|item| item_line(item, role)));
    }

    let mut lines = vec![counts.join(" · ")];
    if body.is_empty() {
        lines.push("nada pendente".to_string());
    } else {
        lines.extend(body);
    }
    lines.join("\n")
}

/// One item, with what helps decide whether to open it: which project, how long
/// it has been waiting, how much conversation it has and its state.
fn item_line(item: &serde_json::Value, role: Role) -> String {
    let number = item
        .get("number")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let title = item
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("(sem título)");

    let mut parts = vec![format!("#{number} {}", truncate(title, 34))];

    if let Some(repo) = item
        .get("repository")
        .and_then(|repo| repo.get("nameWithOwner"))
        .and_then(serde_json::Value::as_str)
    {
        parts.push(repo.to_string());
    }

    // Quem escreveu interessa quando o item não é seu.
    if matches!(role, Role::Review | Role::Issue)
        && let Some(author) = item
            .get("author")
            .and_then(|author| author.get("login"))
            .and_then(serde_json::Value::as_str)
    {
        parts.push(author.to_string());
    }

    if let Some(age) = item
        .get("updatedAt")
        .and_then(serde_json::Value::as_str)
        .and_then(relative_age_iso)
    {
        parts.push(age);
    }

    let comments = item
        .get("comments")
        .and_then(|comments| comments.get("totalCount"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if comments > 0 {
        parts.push(if comments == 1 {
            "1 comentário".to_string()
        } else {
            format!("{comments} comentários")
        });
    }

    if item
        .get("isDraft")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        parts.push("rascunho".to_string());
    }

    if let Some(decision) = item
        .get("reviewDecision")
        .and_then(serde_json::Value::as_str)
    {
        parts.push(
            match decision {
                "APPROVED" => "aprovado",
                "CHANGES_REQUESTED" => "mudanças pedidas",
                "REVIEW_REQUIRED" => "aguardando revisão",
                other => other,
            }
            .to_string(),
        );
    }

    parts.join(" · ")
}

fn truncate(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_string();
    }
    format!(
        "{}…",
        chars[..max.saturating_sub(1)].iter().collect::<String>()
    )
}

fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(HTTP_TIMEOUT))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    agent
        .get(url)
        .call()
        .map_err(|error| format!("[erro] {error}"))?
        .body_mut()
        .read_json::<T>()
        .map_err(|error| format!("[erro] resposta inesperada: {error}"))
}

fn description(code: u32) -> &'static str {
    match code {
        0 => "céu limpo",
        1 => "predomínio de sol",
        2 => "parcialmente nublado",
        3 => "nublado",
        45 => "névoa",
        48 => "névoa com geada",
        51 => "garoa leve",
        53 => "garoa",
        55 => "garoa forte",
        56 | 57 => "garoa congelante",
        61 => "chuva leve",
        63 => "chuva",
        65 => "chuva forte",
        66 | 67 => "chuva congelante",
        71 => "neve leve",
        73 => "neve",
        75 => "neve forte",
        77 => "grãos de neve",
        80 => "pancadas leves",
        81 => "pancadas",
        82 => "pancadas fortes",
        85 | 86 => "pancadas de neve",
        95 => "trovoada",
        96 | 99 => "trovoada com granizo",
        _ => "tempo indefinido",
    }
}

#[derive(Deserialize)]
struct GeocodeResponse {
    #[serde(default)]
    results: Option<Vec<GeocodeResult>>,
}

#[derive(Deserialize)]
struct GeocodeResult {
    latitude: f64,
    longitude: f64,
}

#[derive(Deserialize)]
struct Forecast {
    current: Current,
    daily: Daily,
}

#[derive(Deserialize)]
struct Current {
    #[serde(default)]
    temperature_2m: Option<f64>,
    #[serde(default)]
    relative_humidity_2m: Option<f64>,
    #[serde(default)]
    apparent_temperature: Option<f64>,
    #[serde(default)]
    weather_code: Option<u32>,
    #[serde(default)]
    wind_speed_10m: Option<f64>,
}

#[derive(Deserialize)]
struct Daily {
    #[serde(default)]
    time: Vec<String>,
    #[serde(default)]
    temperature_2m_max: Vec<Option<f64>>,
    #[serde(default)]
    temperature_2m_min: Vec<Option<f64>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_fills_proportionally_and_never_overflows() {
        assert_eq!(bar(0.0), "░".repeat(BAR_CELLS));
        assert_eq!(bar(100.0), "█".repeat(BAR_CELLS));
        assert_eq!(bar(150.0), "█".repeat(BAR_CELLS));
        assert_eq!(bar(50.0).chars().count(), BAR_CELLS);
    }

    #[test]
    fn memory_is_reported_in_gigabytes() {
        assert_eq!(gigabytes(0), "0.0 GB");
        assert_eq!(gigabytes(16 * 1024 * 1024 * 1024), "16.0 GB");
    }

    #[test]
    fn cities_are_percent_encoded_for_the_query_string() {
        assert_eq!(urlencode("Sao Paulo"), "Sao%20Paulo");
        assert_eq!(urlencode("Porto Alegre"), "Porto%20Alegre");
        assert!(urlencode("São Paulo").contains("%C3%A3"));
    }

    #[test]
    fn weather_codes_have_readable_descriptions() {
        assert_eq!(description(0), "céu limpo");
        assert_eq!(description(61), "chuva leve");
        assert_eq!(description(95), "trovoada");
        assert_eq!(description(1234), "tempo indefinido");
    }

    #[test]
    fn the_units_are_read_from_the_config() {
        let metric = WeatherConfig::default();
        assert_eq!(Units::from_config(&metric), Units::Metric);

        let imperial = WeatherConfig {
            units: Some("Imperial".to_string()),
            ..WeatherConfig::default()
        };
        assert_eq!(Units::from_config(&imperial), Units::Imperial);
        assert_eq!(Units::Imperial.temperature(), "°F");
        assert_eq!(Units::Metric.wind(), "km/h");
    }

    #[test]
    fn the_first_two_days_are_named_instead_of_dated() {
        assert_eq!(day_label("2026-06-07", 0), "hoje");
        assert_eq!(day_label("2026-06-08", 1), "amanhã");
        // 2026-06-09 is a Tuesday.
        assert_eq!(day_label("2026-06-09", 2), "ter");
    }

    #[test]
    fn coordinates_come_from_the_config_without_a_request() {
        let config = WeatherConfig {
            lat: Some(-25.43),
            lon: Some(-49.27),
            ..WeatherConfig::default()
        };
        assert_eq!(coordinates(&config), Ok((-25.43, -49.27)));
    }

    #[test]
    fn a_weather_widget_without_a_location_reports_what_is_missing() {
        let output = weather(&WeatherConfig::default());
        assert!(output.starts_with("[erro]"), "{output}");
        assert!(output.contains("city"), "{output}");
    }

    #[test]
    fn an_unknown_widget_kind_is_reported_instead_of_panicking() {
        let config = WidgetConfig {
            kind: "planilha".to_string(),
            title: None,
            command: Vec::new(),
            refresh_secs: 900,
            timeout_secs: 30,
            weather: WeatherConfig::default(),
            gh: GhConfig::default(),
            git: GitConfig::default(),
            http: HttpConfig::default(),
            disk: DiskConfig::default(),
            net: NetConfig::default(),
            commits: CommitsConfig::default(),
        };
        let output = run_widget(
            &config,
            "default",
            Duration::from_secs(5),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        assert!(output.contains("desconhecido"), "{output}");
    }

    #[test]
    fn the_git_status_is_read_from_a_captured_output() {
        // Saída real de `git status --porcelain=v2 --branch --show-stash`.
        let output = "# branch.oid abc123\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +2 -1\n# stash 3\n1 .M N... 100644 100644 100644 aaa bbb src/app.rs\n? notas.md\n";

        let parsed = parse_git_status(output);
        assert!(parsed.starts_with("main * ↑2 ↓1 stash 3"), "{parsed}");
        assert!(parsed.contains("2 arquivos pendentes"), "{parsed}");
    }

    #[test]
    fn a_clean_branch_without_upstream_still_reads() {
        let parsed = parse_git_status("# branch.oid (initial)\n# branch.head main\n");
        assert_eq!(parsed, "main\nlimpo");
    }

    #[test]
    fn one_pending_file_is_written_in_the_singular() {
        let parsed = parse_git_status("# branch.head main\n? so-um.md\n");
        assert!(parsed.contains("1 arquivo pendente"), "{parsed}");
    }

    #[test]
    fn json_path_walks_objects_and_indexes() {
        let body = r#"{"status":"ok","data":{"items":[{"name":"primeiro"},{"name":"segundo"}]},"count":7}"#;

        assert_eq!(json_path(body, ".status").as_deref(), Some("ok"));
        assert_eq!(
            json_path(body, "data.items.1.name").as_deref(),
            Some("segundo")
        );
        assert_eq!(json_path(body, ".count").as_deref(), Some("7"));
        assert_eq!(json_path(body, ".nao.existe"), None);
        assert_eq!(json_path("nao e json", ".x"), None);
    }

    #[test]
    fn sizes_pick_the_unit_that_fits() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KB");
        assert_eq!(human_bytes(16 * 1024 * 1024), "16.0 MB");
        assert_eq!(human_rate(1536.0), "1.5 KB/s");
        assert_eq!(human_rate(-1.0), "0 B/s");
    }

    #[test]
    fn the_http_widget_needs_a_url_before_touching_the_network() {
        let output = http(&HttpConfig::default());
        assert!(output.starts_with("[erro]"), "{output}");
        assert!(output.contains("url"), "{output}");
    }

    #[test]
    fn a_widget_kind_that_needs_no_config_still_reports_something() {
        // Disco e rede leem a máquina: o que importa é não devolver erro.
        let disk = run_widget(
            &WidgetConfig {
                kind: "disk".to_string(),
                ..WidgetConfig::default()
            },
            "default",
            Duration::from_secs(10),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(disk.contains('%'), "{disk}");

        let net = run_widget(
            &WidgetConfig {
                kind: "net".to_string(),
                ..WidgetConfig::default()
            },
            "default",
            Duration::from_secs(10),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(net.contains("recebidos"), "{net}");
    }

    #[test]
    fn the_github_widget_makes_a_single_call() {
        let command = github_command(3);

        assert_eq!(command[0], "gh");
        assert_eq!(command[1], "api");
        assert_eq!(command[2], "graphql");
        assert!(
            !command.iter().any(|arg| arg == "search"),
            "não deveria haver busca por CLI: {command:?}"
        );
        assert!(
            command.iter().any(|arg| arg.contains("review-requested")),
            "{command:?}"
        );
    }

    #[test]
    fn the_github_command_asks_for_what_the_layout_needs() {
        let query = github_command(2).pop().expect("a consulta");

        for field in [
            "issueCount",
            "reviewDecision",
            "isDraft",
            "updatedAt",
            "totalCount",
            "nameWithOwner",
            "login",
        ] {
            assert!(
                query.contains(field),
                "faltou {field} na consulta:\n{query}"
            );
        }
        assert!(query.contains("first: 2"), "{query}");
    }

    #[test]
    fn the_issue_fragment_avoids_fields_only_pull_requests_have() {
        let query = github_command(2).pop().expect("a consulta");
        let issues = query.split("assigned:").nth(1).expect("a busca de issues");

        for field in ["isDraft", "reviewDecision"] {
            assert!(
                !issues.contains(field),
                "uma issue não tem {field}:\\n{issues}"
            );
        }
    }

    #[test]
    fn pending_items_are_grouped_under_their_category() {
        let payload = serde_json::json!({
            "reviewRequested": {
                "issueCount": 1,
                "nodes": [{
                    "number": 12,
                    "title": "fix: layout do split",
                    "updatedAt": "2026-10-15T12:00:00Z",
                    "comments": { "totalCount": 2 },
                    "author": { "login": "fulano" },
                    "repository": { "nameWithOwner": "acme/app" }
                }]
            },
            "authored": { "issueCount": 0, "nodes": [] },
            "assigned": { "issueCount": 2, "nodes": [] }
        });

        let block = format_pending(&payload, 2);
        let lines: Vec<&str> = block.lines().collect();

        assert_eq!(lines[0], "1 revisão · 0 meus PRs · 2 issues");
        assert_eq!(lines[1], "revisões");
        assert!(lines[2].starts_with("#12 fix: layout do split"), "{block}");
        assert!(lines[2].contains("acme/app"), "{block}");
        assert!(lines[2].contains("fulano"), "{block}");
        assert!(lines[2].contains("2 comentários"), "{block}");
        // Uma categoria vazia não ganha cabeçalho nem linha.
        assert!(!block.contains("meus PRs\n"), "{block}");
    }

    #[test]
    fn nothing_pending_is_said_plainly() {
        let block = format_pending(&serde_json::json!({}), 3);
        assert_eq!(block, "0 revisões · 0 meus PRs · 0 issues\nnada pendente");
    }

    #[test]
    fn my_own_prs_show_their_state_instead_of_my_name() {
        let item = serde_json::json!({
            "number": 7,
            "title": "feat: temas",
            "isDraft": true,
            "reviewDecision": "CHANGES_REQUESTED",
            "updatedAt": "2026-10-17T00:00:00Z",
            "comments": { "totalCount": 1 },
            "author": { "login": "eu" },
            "repository": { "nameWithOwner": "acme/app" }
        });

        let line = item_line(&item, Role::Authored);
        assert!(line.contains("rascunho"), "{line}");
        assert!(line.contains("mudanças pedidas"), "{line}");
        assert!(line.contains("1 comentário"), "{line}");
        assert!(
            !line.contains("eu"),
            "o autor não interessa no seu próprio PR: {line}"
        );
    }

    #[test]
    fn items_are_truncated_so_the_line_survives_a_narrow_column() {
        let item = serde_json::json!({
            "number": 182,
            "title": "corrige o alinhamento do relógio quando a janela é estreita",
            "repository": { "nameWithOwner": "minha-org/dashboard-de-vendas" }
        });

        let line = item_line(&item, Role::Review);
        assert!(line.starts_with("#182 "), "{line}");
        assert!(line.contains('…'), "deveria truncar: {line}");
        assert!(line.contains("minha-org/dashboard-de-vendas"), "{line}");
    }

    #[test]
    fn an_item_without_a_title_still_renders() {
        let item = serde_json::json!({ "number": 7 });
        assert!(item_line(&item, Role::Issue).starts_with("#7 (sem título)"));
    }

    #[test]
    fn ages_read_in_the_shortest_useful_form() {
        let now = Local::now().fixed_offset();
        let age = |seconds: i64| relative_age(now - chrono::Duration::seconds(seconds));

        assert_eq!(age(10), "agora");
        assert_eq!(age(5 * 60), "5 min");
        assert_eq!(age(3 * 3600), "3 h");
        assert_eq!(age(2 * 86_400), "2 d");
        assert_eq!(age(60 * 86_400), "2 meses");
        assert_eq!(age(400 * 86_400), "1 ano");
    }

    #[test]
    fn a_commit_line_is_parsed_from_git_output() {
        let line = "7fcff3c|2026-05-25T22:46:28-04:00|feat: sistema completo de notificacoes";
        let commit = parse_commit(line, Path::new("C:/dev/loja-virtual")).expect("um commit");

        assert_eq!(commit.project, "loja-virtual");
        assert_eq!(commit.subject, "feat: sistema completo de notificacoes");
        assert_eq!(commit.at.to_string(), "2026-05-25 22:46:28 -04:00");
    }

    #[test]
    fn a_malformed_commit_line_is_skipped_instead_of_panicking() {
        assert!(parse_commit("sem separador", Path::new("C:/dev/x")).is_none());
        assert!(parse_commit("abc|nao-e-data|assunto", Path::new("C:/dev/x")).is_none());
        assert!(parse_commit("abc|2026-05-25T22:46:28-04:00", Path::new("C:/dev/x")).is_none());
    }

    #[test]
    fn a_leading_tilde_expands_to_the_home_directory() {
        if let Some(home) = dirs::home_dir() {
            assert_eq!(expand_tilde("~/projetos"), home.join("projetos"));
        }
        assert_eq!(expand_tilde("C:/dev"), PathBuf::from("C:/dev"));
        assert_eq!(expand_tilde("  C:/dev  "), PathBuf::from("C:/dev"));
    }

    #[test]
    fn repositories_are_found_under_a_root() {
        // Uma árvore de mentira: dois repositórios na raiz, um mais fundo e uma
        // pasta que não é repositório.
        let base = std::env::temp_dir().join(format!("my-dev-clock-repos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        for repo in ["um", "dois", "ninho/tres"] {
            std::fs::create_dir_all(base.join(repo).join(".git")).expect("cria a árvore");
        }
        std::fs::create_dir_all(base.join("solto")).expect("cria a pasta solta");

        let mut found: Vec<PathBuf> = Vec::new();
        walk_for_repos(&base, 2, &mut found);
        found.sort();

        let names: Vec<String> = found
            .iter()
            .map(|path| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert_eq!(names, vec!["dois", "tres", "um"], "{names:?}");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_commits_widget_asks_for_a_place_to_look() {
        let output = commits(
            &CommitsConfig::default(),
            Duration::from_secs(5),
            Arc::new(AtomicBool::new(false)),
            "default",
        );

        assert!(output.starts_with("[erro]"), "{output}");
        assert!(output.contains("roots"), "{output}");
    }
}
