//! Layout settings: defaults, the `config.toml` file and command line values.
//!
//! Precedence, highest first: command line, config file, built-in defaults.
//!
//! The defaults are a classic thesis layout: A4
//! paper, Times New Roman 12 pt, 1.5 line spacing, a 4 cm binding (inside)
//! margin and 2.5 cm on the other three sides. Nothing, page numbers
//! included, is printed outside the margins.
//!
//! ```toml
//! # ~/.config/epub2pdf/config.toml  (Windows: %APPDATA%\epub2pdf\config.toml)
//! size = "A4"                 # A4, A5, A6, B5, Letter
//! font = "Times New Roman"
//! font_size = 12.0            # points
//! line_spacing = 1.5          # 1 = single, 1.5, 2 = double
//! margins = "25,40,25,25"     # mm: top,inside,outside,bottom
//!                             # (or top,sides,bottom, or one value)
//! ```
//!
//! `epub2pdf set` reads and writes this file (see [`set_values`]).

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::t;

/// The default font.
pub const DEFAULT_FONT: &str = "Times New Roman";
/// Embedded, metric-compatible stand-in used when Times New Roman is missing.
pub const FALLBACK_FONT: &str = "Liberation Serif";
/// Embedded font that covers the remaining scripts and symbols.
pub const LAST_RESORT_FONT: &str = "DejaVu Serif";

/// Final layout settings used for rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub paper: Paper,
    pub font: String,
    /// Body text size in points.
    pub font_size: f64,
    /// Line spacing as in word processors: 1 single, 1.5, 2 double.
    pub line_spacing: f64,
    pub margins: Margins,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            paper: Paper::A4,
            font: DEFAULT_FONT.to_string(),
            font_size: 12.0,
            line_spacing: 1.5,
            margins: Margins {
                top: 25.0,
                inside: 40.0,
                outside: 25.0,
                bottom: 25.0,
            },
        }
    }
}

/// Page margins in millimetres. `inside` is the binding side: the left edge
/// of odd (right-hand) pages and the right edge of even pages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Margins {
    pub top: f64,
    pub inside: f64,
    pub outside: f64,
    pub bottom: f64,
}

impl fmt::Display for Margins {
    /// Shortest form that [`parse_margins`] reads back.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Margins {
            top,
            inside,
            outside,
            bottom,
        } = *self;
        if top == inside && inside == outside && outside == bottom {
            write!(f, "{top}")
        } else if inside == outside {
            write!(f, "{top},{inside},{bottom}")
        } else {
            write!(f, "{top},{inside},{outside},{bottom}")
        }
    }
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

    pub fn name(self) -> &'static str {
        match self {
            Paper::A4 => "A4",
            Paper::A5 => "A5",
            Paper::A6 => "A6",
            Paper::B5 => "B5",
            Paper::Letter => "Letter",
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
    pub line_spacing: Option<f64>,
    pub margins: Option<String>,
}

/// Contents of `config.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileConfig {
    pub size: Option<String>,
    pub font: Option<String>,
    pub font_size: Option<f64>,
    pub line_spacing: Option<f64>,
    pub margins: Option<String>,
}

#[derive(Debug)]
pub enum ConfigError {
    Read(PathBuf, std::io::Error),
    Write(PathBuf, std::io::Error),
    Parse(PathBuf, String),
    InvalidSize(String),
    InvalidFontSize(f64),
    InvalidLineSpacing(f64),
    InvalidMargins(String),
    InvalidNumber(String, String),
    UnknownKey(String),
    NoConfigDir,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            ConfigError::Read(p, e) => t!(
                "ayar dosyası okunamadı ({}): {e}",
                "cannot read config file ({}): {e}",
                p.display()
            ),
            ConfigError::Write(p, e) => t!(
                "ayar dosyası yazılamadı ({}): {e}",
                "cannot write config file ({}): {e}",
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
            ConfigError::InvalidLineSpacing(v) => t!(
                "satır aralığı {v} geçersiz; 0.8 ile 3 arasında olmalı (1 tek, 1.5, 2 çift)",
                "line spacing {v} is invalid; it must be between 0.8 and 3 (1 single, 1.5, 2 double)"
            ),
            ConfigError::InvalidMargins(s) => t!(
                "kenar boşluğu '{s}' geçersiz; '25', 'üst,yan,alt' ya da 'üst,iç,dış,alt' biçiminde 0-60 mm olmalı",
                "margins '{s}' are invalid; use '25', 'top,sides,bottom' or 'top,inside,outside,bottom', each 0-60 mm"
            ),
            ConfigError::InvalidNumber(key, v) => t!(
                "{key} için '{v}' bir sayı değil",
                "'{v}' is not a number (for {key})"
            ),
            ConfigError::UnknownKey(k) => t!(
                "bilinmeyen ayar '{k}' (ayarlar: {})",
                "unknown setting '{k}' (settings: {})",
                Key::NAMES
            ),
            ConfigError::NoConfigDir => t!(
                "kullanıcı ayar klasörü bulunamadı; --ayar ile bir dosya verin",
                "no user config directory found; give a file with --config"
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

/// Reads and parses a config file.
pub fn read_file(path: &Path) -> Result<FileConfig, ConfigError> {
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
    if let Some(spacing) = overrides.line_spacing.or(file.line_spacing) {
        if !(0.8..=3.0).contains(&spacing) {
            return Err(ConfigError::InvalidLineSpacing(spacing));
        }
        settings.line_spacing = spacing;
    }
    if let Some(margins) = overrides.margins.as_ref().or(file.margins.as_ref()) {
        settings.margins =
            parse_margins(margins).ok_or_else(|| ConfigError::InvalidMargins(margins.clone()))?;
    }
    Ok(settings)
}

/// Parses margins in mm: `"25"` (all sides), `"18,16,20"` (top, sides,
/// bottom) or `"25,40,25,25"` (top, inside, outside, bottom).
pub fn parse_margins(text: &str) -> Option<Margins> {
    let values: Vec<f64> = text
        .split(',')
        .map(|v| v.trim().trim_end_matches("mm").trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .ok()?;
    if values.iter().any(|v| !(0.0..=60.0).contains(v)) {
        return None;
    }
    let (top, inside, outside, bottom) = match values.as_slice() {
        [all] => (*all, *all, *all, *all),
        [top, sides, bottom] => (*top, *sides, *sides, *bottom),
        [top, inside, outside, bottom] => (*top, *inside, *outside, *bottom),
        _ => return None,
    };
    Some(Margins {
        top,
        inside,
        outside,
        bottom,
    })
}

// --- `epub2pdf set` ---------------------------------------------------------

/// A setting that `epub2pdf set` can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Size,
    Font,
    FontSize,
    LineSpacing,
    Margins,
}

impl Key {
    pub const ALL: [Key; 5] = [
        Key::Size,
        Key::Font,
        Key::FontSize,
        Key::LineSpacing,
        Key::Margins,
    ];
    pub const NAMES: &'static str =
        "size/boyut, font/yazi-tipi, font-size/punto, line-spacing/satir-araligi, margins/kenar";

    /// Accepts the English and Turkish names, with `-`, `_` or ASCII letters.
    pub fn parse(name: &str) -> Option<Key> {
        let name: String = name
            .trim()
            .to_lowercase()
            .chars()
            .map(|c| match c {
                '_' => '-',
                'ı' => 'i',
                'ş' => 's',
                'ğ' => 'g',
                'ü' => 'u',
                'ö' => 'o',
                'ç' => 'c',
                c => c,
            })
            .collect();
        match name.as_str() {
            "size" | "boyut" | "paper" => Some(Key::Size),
            "font" | "yazi-tipi" => Some(Key::Font),
            "font-size" | "punto" => Some(Key::FontSize),
            "line-spacing" | "satir-araligi" => Some(Key::LineSpacing),
            "margins" | "margin" | "kenar" => Some(Key::Margins),
            _ => None,
        }
    }

    /// Name of the key in `config.toml`.
    pub fn toml_name(self) -> &'static str {
        match self {
            Key::Size => "size",
            Key::Font => "font",
            Key::FontSize => "font_size",
            Key::LineSpacing => "line_spacing",
            Key::Margins => "margins",
        }
    }

    /// Name shown to the user in the interface language.
    pub fn label(self) -> String {
        match self {
            Key::Size => t!("boyut", "size"),
            Key::Font => t!("yazı tipi", "font"),
            Key::FontSize => t!("punto", "font-size"),
            Key::LineSpacing => t!("satır aralığı", "line-spacing"),
            Key::Margins => t!("kenar", "margins"),
        }
    }

    /// The value of this key in `settings`, formatted for display.
    pub fn display(self, settings: &Settings) -> String {
        match self {
            Key::Size => settings.paper.name().to_string(),
            Key::Font => settings.font.clone(),
            Key::FontSize => format!("{} pt", settings.font_size),
            Key::LineSpacing => settings.line_spacing.to_string(),
            Key::Margins => {
                let m = &settings.margins;
                if m.inside == m.outside {
                    t!(
                        "{} mm (üst, yan, alt)",
                        "{} mm (top, sides, bottom)",
                        settings.margins
                    )
                } else {
                    t!(
                        "{} mm (üst, iç/cilt, dış, alt)",
                        "{} mm (top, inside/binding, outside, bottom)",
                        settings.margins
                    )
                }
            }
        }
    }

    /// Whether the config file sets this key.
    pub fn is_set_in(self, file: &FileConfig) -> bool {
        match self {
            Key::Size => file.size.is_some(),
            Key::Font => file.font.is_some(),
            Key::FontSize => file.font_size.is_some(),
            Key::LineSpacing => file.line_spacing.is_some(),
            Key::Margins => file.margins.is_some(),
        }
    }

    fn toml_value(self, value: &str) -> Result<toml::Value, ConfigError> {
        match self {
            Key::FontSize | Key::LineSpacing => {
                // "10,5" is the Turkish way of writing 10.5.
                let number = value.trim().replace(',', ".");
                number
                    .parse::<f64>()
                    .map(toml::Value::Float)
                    .map_err(|_| ConfigError::InvalidNumber(self.label(), value.to_string()))
            }
            _ => Ok(toml::Value::String(value.trim().to_string())),
        }
    }
}

/// Words that remove a key from the file, restoring the default.
pub fn is_default_word(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "default" | "varsayılan" | "varsayilan"
    )
}

/// Applies `key = value` pairs to the config file at `path`, creating it if
/// needed. A value of `default`/`varsayılan` removes the key. The file is
/// only written when every value is valid. Comments in the file are not
/// kept.
pub fn set_values(path: &Path, pairs: &[(Key, String)]) -> Result<Settings, ConfigError> {
    let mut table: toml::Table = if path.exists() {
        let text =
            std::fs::read_to_string(path).map_err(|e| ConfigError::Read(path.to_path_buf(), e))?;
        toml::from_str(&text).map_err(|e| ConfigError::Parse(path.to_path_buf(), e.to_string()))?
    } else {
        toml::Table::new()
    };
    for (key, value) in pairs {
        if is_default_word(value) {
            table.remove(key.toml_name());
        } else {
            table.insert(key.toml_name().to_string(), key.toml_value(value)?);
        }
    }

    // Validate the whole result before touching the file.
    let text = toml::to_string(&table).expect("a table always serialises");
    let file: FileConfig =
        toml::from_str(&text).map_err(|e| ConfigError::Parse(path.to_path_buf(), e.to_string()))?;
    let settings = combine(&file, &Overrides::default())?;

    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| ConfigError::Write(path.to_path_buf(), e))?;
    }
    std::fs::write(path, text).map_err(|e| ConfigError::Write(path.to_path_buf(), e))?;
    Ok(settings)
}
