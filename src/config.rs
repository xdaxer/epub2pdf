//! Layout settings: defaults, the `config.toml` file and command line values.
//!
//! Precedence, highest first: command line, config file, built-in defaults.
//! Defaults: A5 paper, DejaVu Serif at 10.5 pt and 18/16/20 mm margins.
//!
//! ```toml
//! # ~/.config/epub2pdf/config.toml  (Windows: %APPDATA%\epub2pdf\config.toml)
//! size = "A5"           # A4, A5, A6, B5, Letter
//! font = "DejaVu Serif"
//! font_size = 10.5      # points
//! margins = "18,16,20"  # mm: top,sides,bottom  (or one value for all)
//! ```

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::t;

pub const DEFAULT_FONT: &str = "DejaVu Serif";

/// Final layout settings used for rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub paper: Paper,
    pub font: String,
    /// Body text size in points.
    pub font_size: f64,
    pub margins: Margins,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            paper: Paper::A5,
            font: DEFAULT_FONT.to_string(),
            font_size: 10.5,
            margins: Margins {
                top: 18.0,
                sides: 16.0,
                bottom: 20.0,
            },
        }
    }
}

/// Page margins in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Margins {
    pub top: f64,
    pub sides: f64,
    pub bottom: f64,
}

/// Supported paper sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paper {
    A4,
    A5,
    A6,
    B5,
    Letter,
}

impl Paper {
    pub const NAMES: &'static str = "A4, A5, A6, B5, Letter";

    pub fn parse(name: &str) -> Option<Paper> {
        match name.trim().to_ascii_lowercase().as_str() {
            "a4" => Some(Paper::A4),
            "a5" => Some(Paper::A5),
            "a6" => Some(Paper::A6),
            "b5" => Some(Paper::B5),
            "letter" | "us-letter" => Some(Paper::Letter),
            _ => None,
        }
    }

    /// Name of the paper in Typst's `page(paper: ..)`.
    pub fn typst_name(self) -> &'static str {
        match self {
            Paper::A4 => "a4",
            Paper::A5 => "a5",
            Paper::A6 => "a6",
            Paper::B5 => "iso-b5",
            Paper::Letter => "us-letter",
        }
    }
}

/// Values given on the command line. `None` means "not given".
#[derive(Clone, Debug, Default)]
pub struct Overrides {
    pub size: Option<String>,
    pub font: Option<String>,
    pub font_size: Option<f64>,
    pub margins: Option<String>,
}

/// Contents of `config.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileConfig {
    pub size: Option<String>,
    pub font: Option<String>,
    pub font_size: Option<f64>,
    pub margins: Option<String>,
}

#[derive(Debug)]
pub enum ConfigError {
    Read(PathBuf, std::io::Error),
    Parse(PathBuf, String),
    InvalidSize(String),
    InvalidFontSize(f64),
    InvalidMargins(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            ConfigError::Read(p, e) => t!(
                "ayar dosyası okunamadı ({}): {e}",
                "cannot read config file ({}): {e}",
                p.display()
            ),
            ConfigError::Parse(p, e) => t!(
                "ayar dosyası hatalı ({}): {e}",
                "invalid config file ({}): {e}",
                p.display()
            ),
            ConfigError::InvalidSize(s) => t!(
                "bilinmeyen sayfa boyutu '{s}' (seçenekler: {})",
                "unknown page size '{s}' (choices: {})",
                Paper::NAMES
            ),
            ConfigError::InvalidFontSize(v) => t!(
                "yazı boyutu {v} geçersiz; 5 ile 30 punto arasında olmalı",
                "font size {v} is invalid; it must be between 5 and 30 pt"
            ),
            ConfigError::InvalidMargins(s) => t!(
                "kenar boşluğu '{s}' geçersiz; '15' ya da 'üst,yan,alt' biçiminde 0-60 mm olmalı",
                "margins '{s}' are invalid; use '15' or 'top,sides,bottom', each 0-60 mm"
            ),
        };
        f.write_str(&text)
    }
}

impl std::error::Error for ConfigError {}

/// Location of the config file: an explicit path, `EPUB2PDF_CONFIG`, or the
/// platform config directory.
pub fn config_path(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return Some(path.to_path_buf());
    }
    if let Some(path) = std::env::var_os("EPUB2PDF_CONFIG") {
        return Some(PathBuf::from(path));
    }
    dirs::config_dir().map(|dir| dir.join("epub2pdf").join("config.toml"))
}

/// Loads the config file (if any) and applies the command line overrides.
///
/// A missing file at the default location is fine; a missing file that was
/// named explicitly is an error.
pub fn load(explicit: Option<&Path>, overrides: &Overrides) -> Result<Settings, ConfigError> {
    let file = match config_path(explicit) {
        Some(path) if path.exists() || explicit.is_some() => read_file(&path)?,
        _ => FileConfig::default(),
    };
    combine(&file, overrides)
}

fn read_file(path: &Path) -> Result<FileConfig, ConfigError> {
    let text =
        std::fs::read_to_string(path).map_err(|e| ConfigError::Read(path.to_path_buf(), e))?;
    toml::from_str(&text).map_err(|e| ConfigError::Parse(path.to_path_buf(), e.to_string()))
}

/// Merges defaults, file values and overrides, validating the result.
pub fn combine(file: &FileConfig, overrides: &Overrides) -> Result<Settings, ConfigError> {
    let mut settings = Settings::default();

    if let Some(size) = overrides.size.as_ref().or(file.size.as_ref()) {
        settings.paper =
            Paper::parse(size).ok_or_else(|| ConfigError::InvalidSize(size.clone()))?;
    }
    if let Some(font) = overrides.font.as_ref().or(file.font.as_ref()) {
        let font = font.trim();
        if !font.is_empty() {
            settings.font = font.to_string();
        }
    }
    if let Some(size) = overrides.font_size.or(file.font_size) {
        if !(5.0..=30.0).contains(&size) {
            return Err(ConfigError::InvalidFontSize(size));
        }
        settings.font_size = size;
    }
    if let Some(margins) = overrides.margins.as_ref().or(file.margins.as_ref()) {
        settings.margins =
            parse_margins(margins).ok_or_else(|| ConfigError::InvalidMargins(margins.clone()))?;
    }
    Ok(settings)
}

/// Parses `"15"` (all sides) or `"18,16,20"` (top, sides, bottom) in mm.
pub fn parse_margins(text: &str) -> Option<Margins> {
    let values: Vec<f64> = text
        .split(',')
        .map(|v| v.trim().trim_end_matches("mm").trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .ok()?;
    if values.iter().any(|v| !(0.0..=60.0).contains(v)) {
        return None;
    }
    match values.as_slice() {
        [all] => Some(Margins {
            top: *all,
            sides: *all,
            bottom: *all,
        }),
        [top, sides, bottom] => Some(Margins {
            top: *top,
            sides: *sides,
            bottom: *bottom,
        }),
        _ => None,
    }
}
