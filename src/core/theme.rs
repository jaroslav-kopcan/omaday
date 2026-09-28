use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

/// The Omarchy palette. Every theme defines all of these keys.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Palette {
    pub mode: String,
    pub accent: String,
    pub selection: String,
    pub muted: String,
    pub background: String,
    pub dark_background: String,
    pub darker_background: String,
    pub lighter_background: String,
    pub foreground: String,
    pub dark_foreground: String,
    pub light_foreground: String,
    pub bright_foreground: String,
    pub red: String,
    pub yellow: String,
    pub orange: String,
    pub green: String,
    pub cyan: String,
    pub blue: String,
    pub magenta: String,
    pub brown: String,
    pub bright_red: String,
    pub bright_yellow: String,
    pub bright_green: String,
    pub bright_cyan: String,
    pub bright_blue: String,
    pub bright_magenta: String,
}

#[derive(Debug)]
pub enum ThemeError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse(String),
    BadColor {
        key: &'static str,
        value: String,
    },
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::Read { path, source } => {
                write!(f, "cannot read {}: {}", path.display(), source)
            }
            ThemeError::Parse(message) => write!(f, "palette is not valid: {message}"),
            ThemeError::BadColor { key, value } => {
                write!(f, "palette key {key} is not a #rrggbb color: {value:?}")
            }
        }
    }
}

impl std::error::Error for ThemeError {}

/// Tokyo Night, used when Omarchy's palette cannot be read.
const BUILTIN: &str = include_str!("../../tests/fixtures/tokyo-night.toml");

/// GTK CSS with `$name` placeholders for palette keys, `$font`, `$size`,
/// `$large`, `$indent` and `$editor_min_height`.
const CSS_TEMPLATE: &str = r#"
window.omaday { background-color: $background; }
window.omaday label,
window.omaday button { font-family: "$font"; }
.omaday-large { font-size: $largept; }
.omaday-body { font-size: $sizept; }

button.month, button.year, button.day {
  background: none;
  border: none;
  border-bottom: 2px solid transparent;
  border-radius: 0;
  box-shadow: none;
  padding: 2px 0;
  min-height: 0;
  min-width: 0;
  color: $dark_foreground;
  outline-offset: 2px;
}
button.month:hover, button.year:hover, button.day:hover { color: $light_foreground; }
button.month:focus-visible, button.year:focus-visible, button.day:focus-visible {
  outline: 1px solid $accent;
}
button.month.selected, button.year.selected {
  color: $bright_foreground;
  border-bottom-color: $accent;
}
button.day.has-note { color: $bright_foreground; }
button.day.today { color: $accent; }
button.day label.number { font-weight: bold; }

.editor-body { margin-left: $indentpx; }
textview.editor, textview.editor text {
  background-color: transparent;
  color: $foreground;
  caret-color: $accent;
}
textview.editor {
  font-family: "$font";
  font-size: $sizept;
  min-height: $editor_min_heightpx;
}
textview.editor text selection {
  background-color: $selection;
  color: $bright_foreground;
}
label.notice { color: $yellow; }
label.startup-error { color: $yellow; font-family: "$font"; font-size: $sizept; }

.fade-top { background: linear-gradient(to bottom, $background, transparent); }
.fade-bottom { background: linear-gradient(to top, $background, transparent); }

scrollbar { background-color: transparent; border: none; }
scrollbar slider { background-color: $muted; min-width: 4px; border-radius: 2px; }
"#;

impl Palette {
    pub fn parse(text: &str) -> Result<Palette, ThemeError> {
        let palette: Palette =
            toml::from_str(text).map_err(|e| ThemeError::Parse(e.to_string()))?;
        for (key, value) in palette.colors() {
            if !is_hex_color(value) {
                return Err(ThemeError::BadColor {
                    key,
                    value: value.to_string(),
                });
            }
        }
        Ok(palette)
    }

    pub fn load(path: &Path) -> Result<Palette, ThemeError> {
        let text = std::fs::read_to_string(path).map_err(|source| ThemeError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Palette::parse(&text)
    }

    pub fn builtin() -> Palette {
        Palette::parse(BUILTIN).expect("the builtin palette is valid; covered by tests")
    }

    /// Every color key with its value, in a fixed order. `mode` is not a color.
    pub fn colors(&self) -> [(&'static str, &str); 25] {
        [
            ("accent", self.accent.as_str()),
            ("selection", self.selection.as_str()),
            ("muted", self.muted.as_str()),
            ("background", self.background.as_str()),
            ("dark_background", self.dark_background.as_str()),
            ("darker_background", self.darker_background.as_str()),
            ("lighter_background", self.lighter_background.as_str()),
            ("foreground", self.foreground.as_str()),
            ("dark_foreground", self.dark_foreground.as_str()),
            ("light_foreground", self.light_foreground.as_str()),
            ("bright_foreground", self.bright_foreground.as_str()),
            ("red", self.red.as_str()),
            ("yellow", self.yellow.as_str()),
            ("orange", self.orange.as_str()),
            ("green", self.green.as_str()),
            ("cyan", self.cyan.as_str()),
            ("blue", self.blue.as_str()),
            ("magenta", self.magenta.as_str()),
            ("brown", self.brown.as_str()),
            ("bright_red", self.bright_red.as_str()),
            ("bright_yellow", self.bright_yellow.as_str()),
            ("bright_green", self.bright_green.as_str()),
            ("bright_cyan", self.bright_cyan.as_str()),
            ("bright_blue", self.bright_blue.as_str()),
            ("bright_magenta", self.bright_magenta.as_str()),
        ]
    }

    /// Build the whole stylesheet. `font` is a fontconfig family name,
    /// `font_size` is in points and is the body size; large labels get two more.
    pub fn to_css(&self, font: &str, font_size: u32) -> String {
        let font = font.replace('"', "");
        let large = font_size + 2;
        let px_per_pt = 1.333_f32;
        // Six lines of text at a 1.4 line height.
        let editor_min_height = (font_size as f32 * px_per_pt * 1.4 * 6.0).round() as u32;
        // Width of a three glyph day number in a monospace face, plus the row spacing.
        let indent = (large as f32 * px_per_pt * 0.6 * 3.0).round() as u32 + 12;

        let mut css = CSS_TEMPLATE.to_string();
        for (key, value) in [
            ("$editor_min_height", editor_min_height),
            ("$indent", indent),
            ("$large", large),
            ("$size", font_size),
        ] {
            css = css.replace(key, &value.to_string());
        }
        css = css.replace("$font", &font);
        for (key, value) in self.colors() {
            css = css.replace(&format!("${key}"), value);
        }
        css
    }
}

/// `#rrggbb`, nothing else. Keeps bad values out of the generated CSS.
pub fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// `$XDG_STATE_HOME/omarchy/current/theme/colors.toml`, default under `~/.local/state`.
pub fn palette_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("state"))
        })?;
    Some(
        base.join("omarchy")
            .join("current")
            .join("theme")
            .join("colors.toml"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKYO: &str = include_str!("../../tests/fixtures/tokyo-night.toml");
    const CATPPUCCIN: &str = include_str!("../../tests/fixtures/catppuccin.toml");

    #[test]
    fn real_theme_files_parse() {
        let p = Palette::parse(TOKYO).unwrap();
        assert_eq!(p.mode, "dark");
        assert_eq!(p.background, "#1a1b26");
        assert_eq!(p.accent, "#7aa2f7");
        assert_eq!(p.bright_foreground, "#c0caf5");
        let c = Palette::parse(CATPPUCCIN).unwrap();
        assert!(is_hex_color(&c.background));
        assert!(is_hex_color(&c.bright_magenta));
    }

    #[test]
    fn builtin_is_tokyo_night() {
        assert_eq!(Palette::builtin(), Palette::parse(TOKYO).unwrap());
    }

    #[test]
    fn missing_key_is_an_error() {
        let text = TOKYO.replace("accent = \"#7aa2f7\"\n", "");
        assert!(matches!(Palette::parse(&text), Err(ThemeError::Parse(_))));
    }

    #[test]
    fn syntax_error_is_an_error() {
        assert!(matches!(
            Palette::parse("mode = "),
            Err(ThemeError::Parse(_))
        ));
    }

    #[test]
    fn bad_color_is_an_error() {
        let text = TOKYO.replace("#7aa2f7", "blue");
        assert!(matches!(
            Palette::parse(&text),
            Err(ThemeError::BadColor { key: "accent", .. })
        ));
    }

    #[test]
    fn extra_keys_are_allowed() {
        let text = format!("{TOKYO}\nfuture_key = \"#000000\"\n");
        assert!(Palette::parse(&text).is_ok());
    }

    #[test]
    fn hex_color_check() {
        assert!(is_hex_color("#1a1b26"));
        assert!(is_hex_color("#ABCDEF"));
        assert!(!is_hex_color("1a1b26"));
        assert!(!is_hex_color("#1a1b2"));
        assert!(!is_hex_color("#1a1b2g"));
        assert!(!is_hex_color(""));
    }

    #[test]
    fn load_reads_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("colors.toml");
        std::fs::write(&path, TOKYO).unwrap();
        assert_eq!(Palette::load(&path).unwrap(), Palette::builtin());
        assert!(matches!(
            Palette::load(&dir.path().join("nope.toml")),
            Err(ThemeError::Read { .. })
        ));
    }

    #[test]
    fn palette_path_ends_with_the_omarchy_file() {
        let path = palette_path().unwrap();
        assert!(
            path.ends_with("omarchy/current/theme/colors.toml"),
            "{}",
            path.display()
        );
    }

    #[test]
    fn css_uses_palette_font_and_sizes() {
        let css = Palette::builtin().to_css("JetBrainsMono Nerd Font", 12);
        assert!(css.contains("background-color: #1a1b26;"), "{css}");
        assert!(css.contains("caret-color: #7aa2f7;"), "{css}");
        assert!(
            css.contains("font-family: \"JetBrainsMono Nerd Font\";"),
            "{css}"
        );
        assert!(css.contains("font-size: 12pt;"), "{css}");
        assert!(css.contains("font-size: 14pt;"), "{css}");
        assert!(css.contains("min-height: 134px;"), "{css}");
        assert!(css.contains("margin-left: 46px;"), "{css}");
        assert!(
            css.contains("button.day.today { color: #7aa2f7; }"),
            "{css}"
        );
        assert!(!css.contains('$'), "unreplaced placeholder in:\n{css}");
    }

    #[test]
    fn css_strips_quotes_from_the_font_name() {
        let css = Palette::builtin().to_css("Bad\"Name", 12);
        assert!(css.contains("font-family: \"BadName\";"), "{css}");
    }
}
