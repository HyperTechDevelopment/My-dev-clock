use ratatui::style::{Color, Modifier, Style};

use crate::config::CustomTheme;

/// A named palette applied to every part of the interface.
///
/// Themes are pure color tables: `named` maps a name to one of them and
/// anything unknown falls back to `default`, which uses the base color given by
/// `--color`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    /// The big block digits.
    pub clock: Style,
    /// The header line above the digits.
    pub header: Style,
    /// Plain widget body text.
    pub widget: Style,
    /// The widget title line.
    pub widget_title: Style,
    /// The bullet in front of a widget title.
    pub widget_marker: Style,
    /// Accent used for bars and highlighted values.
    pub accent: Style,
    /// Background animation: dim on purpose, so it stays behind the digits.
    pub animation: Style,
}

/// The themes registered out of the box, in `Shift+T` cycle order.
pub const BUILTIN: [&str; 6] = ["default", "matrix", "neon", "system", "fire", "ice"];
/// The name that resolves from `[theme.custom]` in the config instead of from
/// the built-in palettes.
pub const CUSTOM: &str = "custom";

pub fn is_custom(name: &str) -> bool {
    name.trim().eq_ignore_ascii_case(CUSTOM)
}

/// Parses a color name or a `#RRGGBB` value, the same way the config and the
/// command line do.
pub fn parse_color(input: &str) -> Result<Color, String> {
    let value = input.trim().to_ascii_lowercase();
    let named = match value.as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" => Some(Color::DarkGray),
        "lightred" => Some(Color::LightRed),
        "lightgreen" => Some(Color::LightGreen),
        "lightyellow" => Some(Color::LightYellow),
        "lightblue" => Some(Color::LightBlue),
        "lightmagenta" => Some(Color::LightMagenta),
        "lightcyan" => Some(Color::LightCyan),
        "white" => Some(Color::White),
        _ => None,
    };
    if let Some(color) = named {
        return Ok(color);
    }

    let hex = value
        .strip_prefix('#')
        .ok_or_else(|| format!("cor inválida: {input}"))?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("cor inválida: {input}"));
    }

    let channel = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&hex[range], 16).map_err(|error| format!("cor inválida: {error}"))
    };
    Ok(Color::Rgb(channel(0..2)?, channel(2..4)?, channel(4..6)?))
}

fn bold(style: Style) -> Style {
    style
        .add_modifier(Modifier::BOLD)
        .remove_modifier(Modifier::DIM)
}

/// The animation has to read as a background, so it is always dimmed.
fn faint(color: Color) -> Style {
    Style::default().fg(color).add_modifier(Modifier::DIM)
}

impl Theme {
    pub fn named(name: &str, base: Style) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "matrix" => Self::matrix(),
            "neon" => Self::neon(),
            "system" => Self::system(),
            "fire" => Self::fire(),
            "ice" => Self::ice(),
            _ => Self::default_palette(base),
        }
    }

    /// Builds a palette from the colors in [theme.custom], taking whatever is
    /// missing from the default palette for the base color. Each color keeps the
    /// modifier the built-in palette had, so a custom animation stays dim.
    pub fn custom(spec: &CustomTheme, base: Style) -> Self {
        let fallback = Self::default_palette(base);
        let pick = |value: &Option<String>, fallback: Style| {
            value
                .as_deref()
                .and_then(|value| parse_color(value).ok())
                .map(|color| fallback.fg(color))
                .unwrap_or(fallback)
        };

        Self {
            clock: pick(&spec.clock, fallback.clock),
            header: pick(&spec.header, fallback.header),
            widget: pick(&spec.widget, fallback.widget),
            widget_title: pick(&spec.title, fallback.widget_title),
            widget_marker: pick(&spec.marker, fallback.widget_marker),
            accent: pick(&spec.accent, fallback.accent),
            animation: pick(&spec.animation, fallback.animation),
        }
    }

    pub fn is_builtin(name: &str) -> bool {
        BUILTIN
            .iter()
            .any(|theme| theme.eq_ignore_ascii_case(name.trim()))
    }

    fn default_palette(base: Style) -> Self {
        Self {
            clock: base,
            header: base,
            widget: base,
            widget_title: bold(base.fg(Color::Gray)),
            widget_marker: bold(base.fg(Color::Cyan)),
            accent: bold(base.fg(Color::LightGreen)),
            animation: faint(base.fg.unwrap_or(Color::Green)),
        }
    }

    /// Green phosphor terminal, the classic falling-code look.
    fn matrix() -> Self {
        Self {
            clock: bold(Style::default().fg(Color::Rgb(0, 255, 70))),
            header: Style::default().fg(Color::Rgb(0, 170, 60)),
            widget: Style::default().fg(Color::Rgb(0, 200, 80)),
            widget_title: bold(Style::default().fg(Color::Rgb(150, 255, 170))),
            widget_marker: bold(Style::default().fg(Color::Rgb(0, 255, 0))),
            accent: bold(Style::default().fg(Color::Rgb(180, 255, 120))),
            animation: faint(Color::Rgb(0, 140, 45)),
        }
    }

    /// Magenta and cyan, high contrast on dark terminals.
    fn neon() -> Self {
        Self {
            clock: bold(Style::default().fg(Color::Rgb(255, 0, 190))),
            header: Style::default().fg(Color::Rgb(0, 235, 235)),
            widget: Style::default().fg(Color::Rgb(190, 150, 255)),
            widget_title: bold(Style::default().fg(Color::Rgb(0, 255, 255))),
            widget_marker: bold(Style::default().fg(Color::Rgb(255, 0, 190))),
            accent: bold(Style::default().fg(Color::Rgb(120, 255, 200))),
            animation: faint(Color::Rgb(120, 60, 170)),
        }
    }

    /// Defers to the terminal's own foreground and background, so it inherits
    /// whatever color scheme is configured.
    fn system() -> Self {
        Self {
            clock: bold(Style::default().fg(Color::Reset)),
            header: Style::default().fg(Color::Reset),
            widget: Style::default().fg(Color::Reset),
            widget_title: bold(Style::default().fg(Color::Reset)),
            widget_marker: Style::default().fg(Color::Reset),
            accent: bold(Style::default().fg(Color::Reset)),
            animation: faint(Color::Reset),
        }
    }

    fn fire() -> Self {
        Self {
            clock: bold(Style::default().fg(Color::Rgb(255, 120, 0))),
            header: Style::default().fg(Color::Rgb(255, 190, 80)),
            widget: Style::default().fg(Color::Rgb(255, 95, 40)),
            widget_title: bold(Style::default().fg(Color::Rgb(255, 225, 130))),
            widget_marker: bold(Style::default().fg(Color::Rgb(255, 60, 0))),
            accent: bold(Style::default().fg(Color::Rgb(255, 170, 40))),
            animation: faint(Color::Rgb(150, 60, 20)),
        }
    }

    fn ice() -> Self {
        Self {
            clock: bold(Style::default().fg(Color::Rgb(120, 220, 255))),
            header: Style::default().fg(Color::Rgb(190, 235, 255)),
            widget: Style::default().fg(Color::Rgb(140, 195, 255)),
            widget_title: bold(Style::default().fg(Color::Rgb(220, 245, 255))),
            widget_marker: bold(Style::default().fg(Color::Rgb(80, 180, 255))),
            accent: bold(Style::default().fg(Color::Rgb(160, 230, 255))),
            animation: faint(Color::Rgb(70, 130, 180)),
        }
    }
}

/// The theme list to cycle through: the built-in order when the config lists
/// nothing, otherwise the configured names, with the initial theme moved to the
/// front so `Shift+T` continues from where the app started.
pub fn cycle_order(configured: &[String], initial: Option<&str>) -> Vec<String> {
    let mut themes: Vec<String> = if configured.is_empty() {
        BUILTIN.iter().map(|theme| (*theme).to_string()).collect()
    } else {
        configured
            .iter()
            .map(|theme| theme.trim().to_string())
            .filter(|theme| !theme.is_empty())
            .collect()
    };
    if themes.is_empty() {
        themes = BUILTIN.iter().map(|theme| (*theme).to_string()).collect();
    }

    if let Some(initial) = initial.map(str::trim).filter(|theme| !theme.is_empty()) {
        themes.retain(|theme| !theme.eq_ignore_ascii_case(initial));
        themes.insert(0, initial.to_string());
    }
    themes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_names_fall_back_to_the_default_palette() {
        let base = Style::default().fg(Color::Green);
        assert_eq!(
            Theme::named("nao-existe", base),
            Theme::default_palette(base)
        );
        assert_eq!(Theme::named("  ", base), Theme::default_palette(base));
    }

    #[test]
    fn every_builtin_name_resolves_to_its_own_palette() {
        let base = Style::default().fg(Color::Green);
        for name in BUILTIN {
            assert!(Theme::is_builtin(name));
            let theme = Theme::named(name, base);
            if name != "default" {
                assert_ne!(theme, Theme::default_palette(base), "{name}");
            }
        }
    }

    #[test]
    fn colors_accept_names_and_hex_values() {
        assert_eq!(parse_color("LightCyan"), Ok(Color::LightCyan));
        assert_eq!(parse_color("#e63946"), Ok(Color::Rgb(230, 57, 70)));
        assert!(parse_color("#xyzxyz").is_err());
        assert!(parse_color("verde").is_err());
        assert!(parse_color("#fff").is_err());
    }

    #[test]
    fn a_custom_palette_takes_every_color_it_is_given() {
        let base = Style::default().fg(Color::Green);
        let spec = CustomTheme {
            clock: Some("#00ff88".to_string()),
            accent: Some("lightyellow".to_string()),
            ..CustomTheme::default()
        };

        let theme = Theme::custom(&spec, base);
        assert_eq!(theme.clock.fg, Some(Color::Rgb(0, 255, 136)));
        assert_eq!(theme.accent.fg, Some(Color::LightYellow));
        // O que não foi dado vem do padrão, com os modificadores preservados.
        assert_eq!(theme.widget, Theme::default_palette(base).widget);
        assert!(theme.animation.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn a_broken_color_in_the_custom_palette_is_ignored() {
        let base = Style::default().fg(Color::Green);
        let spec = CustomTheme {
            clock: Some("nao-e-cor".to_string()),
            ..CustomTheme::default()
        };

        let theme = Theme::custom(&spec, base);
        assert_eq!(theme.clock, Theme::default_palette(base).clock);
    }

    #[test]
    fn the_custom_name_is_recognised() {
        assert!(is_custom(" custom "));
        assert!(is_custom("CUSTOM"));
        assert!(!is_custom("matrix"));
    }

    #[test]
    fn every_palette_dims_its_animation() {
        let base = Style::default().fg(Color::Green);
        for name in BUILTIN {
            let theme = Theme::named(name, base);
            assert!(
                theme.animation.add_modifier.contains(Modifier::DIM),
                "o tema {name} não escureceu a animação"
            );
        }
    }

    #[test]
    fn cycle_order_defaults_to_the_builtin_list() {
        assert_eq!(cycle_order(&[], None).len(), BUILTIN.len());
    }

    #[test]
    fn cycle_order_puts_the_initial_theme_first_without_duplicating_it() {
        let configured = vec!["matrix".to_string(), "neon".to_string()];
        assert_eq!(
            cycle_order(&configured, Some("neon")),
            vec!["neon", "matrix"]
        );

        let configured: Vec<String> = BUILTIN.iter().map(|theme| (*theme).to_string()).collect();
        assert_eq!(
            cycle_order(&configured, Some("fire")),
            vec!["fire", "default", "matrix", "neon", "system", "ice"]
        );
    }
}
