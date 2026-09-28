use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

/// Settings from `~/.config/omaday/config.toml`, with defaults for anything missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub vault: PathBuf,
    pub font: String,
    pub font_size: u32,
}

#[derive(Debug)]
pub enum ConfigError {
    NoHome,
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NoHome => write!(f, "HOME is not set, cannot find the config file"),
            ConfigError::Read { path, source } => {
                write!(f, "cannot read {}: {}", path.display(), source)
            }
            ConfigError::Parse { path, message } => {
                write!(f, "config error in {}: {}", path.display(), message)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

const MIN_FONT_SIZE: u32 = 6;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    vault: Option<String>,
    font: Option<String>,
    font_size: Option<u32>,
}

impl Config {
    pub fn defaults(home: &Path) -> Config {
        Config {
            vault: home.join("vault"),
            font: "monospace".to_string(),
            font_size: 12,
        }
    }

    /// Defaults for when the home directory is unknown. Only used to style an error screen.
    pub fn fallback() -> Config {
        match home_dir() {
            Some(home) => Config::defaults(&home),
            None => Config {
                vault: PathBuf::from("/"),
                font: "monospace".to_string(),
                font_size: 12,
            },
        }
    }

    /// Parse config text. The error string is meant for the screen.
    pub fn parse(text: &str, home: &Path) -> Result<Config, String> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut cfg = Config::defaults(home);
        if let Some(vault) = raw.vault {
            cfg.vault = expand_tilde(&vault, home);
        }
        if let Some(font) = raw.font {
            cfg.font = font;
        }
        if let Some(size) = raw.font_size {
            if size < MIN_FONT_SIZE {
                return Err(format!(
                    "font_size must be at least {MIN_FONT_SIZE}, got {size}"
                ));
            }
            cfg.font_size = size;
        }
        Ok(cfg)
    }

    /// Read the config file if it exists, else defaults.
    pub fn load() -> Result<Config, ConfigError> {
        let home = home_dir().ok_or(ConfigError::NoHome)?;
        let path = config_path(&home);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                Config::parse(&text, &home).map_err(|message| ConfigError::Parse { path, message })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::defaults(&home)),
            Err(source) => Err(ConfigError::Read { path, source }),
        }
    }
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// `$XDG_CONFIG_HOME/omaday/config.toml`, default `~/.config/omaday/config.toml`.
pub fn config_path(home: &Path) -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    base.join("omaday").join("config.toml")
}

/// Expand a leading `~` or `~/` to the home directory. Other forms are left alone.
pub fn expand_tilde(value: &str, home: &Path) -> PathBuf {
    if value == "~" {
        home.to_path_buf()
    } else if let Some(rest) = value.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("/home/test")
    }

    #[test]
    fn empty_text_gives_defaults() {
        let cfg = Config::parse("", &home()).unwrap();
        assert_eq!(
            cfg,
            Config {
                vault: PathBuf::from("/home/test/vault"),
                font: "monospace".into(),
                font_size: 12,
            }
        );
        assert_eq!(cfg, Config::defaults(&home()));
    }

    #[test]
    fn vault_tilde_is_expanded() {
        let cfg = Config::parse("vault = \"~/notes\"", &home()).unwrap();
        assert_eq!(cfg.vault, PathBuf::from("/home/test/notes"));
        let cfg = Config::parse("vault = \"/srv/notes\"", &home()).unwrap();
        assert_eq!(cfg.vault, PathBuf::from("/srv/notes"));
    }

    #[test]
    fn font_keys_override_defaults() {
        let cfg = Config::parse("font = \"Inter\"\nfont_size = 14\n", &home()).unwrap();
        assert_eq!(cfg.font, "Inter");
        assert_eq!(cfg.font_size, 14);
    }

    #[test]
    fn wrong_type_is_an_error() {
        assert!(Config::parse("vault = 3", &home()).is_err());
    }

    #[test]
    fn unknown_key_is_an_error() {
        assert!(Config::parse("nope = 1", &home()).is_err());
    }

    #[test]
    fn syntax_error_is_an_error() {
        assert!(Config::parse("vault = ", &home()).is_err());
    }

    #[test]
    fn tiny_font_size_is_an_error() {
        let err = Config::parse("font_size = 3", &home()).unwrap_err();
        assert!(err.contains("font_size"), "{err}");
    }

    #[test]
    fn expand_tilde_cases() {
        assert_eq!(expand_tilde("~", &home()), PathBuf::from("/home/test"));
        assert_eq!(
            expand_tilde("~/a/b", &home()),
            PathBuf::from("/home/test/a/b")
        );
        assert_eq!(expand_tilde("~user/x", &home()), PathBuf::from("~user/x"));
    }

    #[test]
    fn config_path_is_under_the_home_by_default() {
        let path = config_path(&home());
        assert!(path.ends_with("omaday/config.toml"), "{}", path.display());
    }

    #[test]
    fn fallback_has_the_default_font() {
        let cfg = Config::fallback();
        assert_eq!(cfg.font, "monospace");
        assert_eq!(cfg.font_size, 12);
    }
}
