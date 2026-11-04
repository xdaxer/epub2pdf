//! User interface language selection (Turkish / English).
//!
//! The language is chosen once per process:
//! 1. `EPUB2PDF_LANG` (`tr` or `en`) if set,
//! 2. otherwise the system locale (`tr*` selects Turkish),
//! 3. otherwise English.
//!
//! Messages are written inline with the [`t!`](crate::t) macro, which keeps
//! both translations next to each other in the source.

use std::sync::OnceLock;

/// A supported interface language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Tr,
    En,
}

static LANG: OnceLock<Lang> = OnceLock::new();

/// Returns the interface language of this process.
pub fn lang() -> Lang {
    *LANG.get_or_init(|| {
        let env = std::env::var("EPUB2PDF_LANG").ok();
        let locale = sys_locale::get_locale();
        detect(env.as_deref(), locale.as_deref())
    })
}

/// Picks a language from an explicit setting and a system locale.
///
/// An explicit setting wins when it names a supported language; anything
/// else falls back to the locale. Locales such as `tr`, `tr-TR` or
/// `tr_TR.UTF-8` select Turkish.
pub fn detect(explicit: Option<&str>, locale: Option<&str>) -> Lang {
    if let Some(lang) = explicit.and_then(parse) {
        return lang;
    }
    match locale {
        Some(l) if l.to_ascii_lowercase().starts_with("tr") => Lang::Tr,
        _ => Lang::En,
    }
}

fn parse(value: &str) -> Option<Lang> {
    match value.trim().to_ascii_lowercase().as_str() {
        "tr" | "turkish" | "türkçe" => Some(Lang::Tr),
        "en" | "english" => Some(Lang::En),
        _ => None,
    }
}

/// Formats a message in the current interface language.
///
/// ```
/// let n = 3;
/// let text = epub2pdf::t!("{n} bölüm", "{n} chapters");
/// assert!(text.starts_with('3'));
/// ```
#[macro_export]
macro_rules! t {
    ($tr:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        match $crate::i18n::lang() {
            $crate::i18n::Lang::Tr => format!($tr $(, $arg)*),
            $crate::i18n::Lang::En => format!($en $(, $arg)*),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_setting_wins() {
        assert_eq!(detect(Some("en"), Some("tr_TR.UTF-8")), Lang::En);
        assert_eq!(detect(Some("TR"), Some("en-US")), Lang::Tr);
    }

    #[test]
    fn locale_is_used_when_setting_is_missing_or_unknown() {
        assert_eq!(detect(None, Some("tr-TR")), Lang::Tr);
        assert_eq!(detect(None, Some("tr_TR.UTF-8")), Lang::Tr);
        assert_eq!(detect(Some("de"), Some("tr")), Lang::Tr);
        assert_eq!(detect(None, Some("de-DE")), Lang::En);
        assert_eq!(detect(None, None), Lang::En);
    }
}
