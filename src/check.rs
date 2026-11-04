//! Inspecting a finished PDF (`epub2pdf check`).
//!
//! Reports the page count, title and author, the number of bookmarks (table
//! of contents) and a text sample to see whether Turkish letters survived.
//! Works on any PDF, not only ours.

use std::fmt;
use std::path::Path;

use lopdf::{Document, Object};

use crate::t;

#[derive(Debug, Clone)]
pub struct Report {
    pub pages: usize,
    pub title: Option<String>,
    pub author: Option<String>,
    /// Number of bookmarks, counted at every level.
    pub outline_entries: usize,
    /// 1-based page number of the text sample.
    pub sample_page: usize,
    pub sample_text: String,
}

#[derive(Debug)]
pub struct CheckError(String);

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&t!("PDF okunamadı: {}", "cannot read PDF: {}", self.0))
    }
}

impl std::error::Error for CheckError {}

/// Reads the PDF at `path`.
pub fn inspect(path: &Path) -> Result<Report, CheckError> {
    let doc = Document::load(path).map_err(|e| CheckError(e.to_string()))?;
    let pages = doc.get_pages();
    let page_count = pages.len();
    let sample_page = match page_count {
        0 => 0,
        n if n > 13 => 13,
        n => n.div_ceil(2),
    };
    let sample_text = if sample_page > 0 {
        doc.extract_text(&[sample_page as u32]).unwrap_or_default()
    } else {
        String::new()
    };
    Ok(Report {
        pages: page_count,
        title: info_string(&doc, b"Title"),
        author: info_string(&doc, b"Author"),
        outline_entries: count_outline(&doc),
        sample_page,
        sample_text,
    })
}

fn info_string(doc: &Document, key: &[u8]) -> Option<String> {
    let info = doc.trailer.get_deref(b"Info", doc).ok()?.as_dict().ok()?;
    let value = info.get_deref(key, doc).ok()?;
    let text = lopdf::decode_text_string(value).ok()?;
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn count_outline(doc: &Document) -> usize {
    let Ok(catalog) = doc.catalog() else {
        return 0;
    };
    let Ok(outlines) = catalog
        .get_deref(b"Outlines", doc)
        .and_then(Object::as_dict)
    else {
        return 0;
    };
    let mut count = 0;
    let mut stack: Vec<&Object> = outlines.get(b"First").ok().into_iter().collect();
    let mut guard = 0;
    while let Some(object) = stack.pop() {
        guard += 1;
        if guard > 100_000 {
            break; // Malformed, cyclic outline.
        }
        let Ok((_, item)) = doc.dereference(object) else {
            continue;
        };
        let Ok(item) = item.as_dict() else {
            continue;
        };
        count += 1;
        if let Ok(next) = item.get(b"Next") {
            stack.push(next);
        }
        if let Ok(first) = item.get(b"First") {
            stack.push(first);
        }
    }
    count
}

/// Formats the report for the terminal; the sample is cut to a few lines.
pub fn format(report: &Report) -> String {
    let missing = t!("(yok)", "(none)");
    let sample: String = report
        .sample_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(300)
        .collect();
    let mut out = vec![
        format!("{}: {}", t!("Sayfa", "Pages"), report.pages),
        format!(
            "{}: {}",
            t!("Başlık", "Title"),
            report.title.clone().unwrap_or_else(|| missing.clone())
        ),
        format!(
            "{}: {}",
            t!("Yazar", "Author"),
            report.author.clone().unwrap_or_else(|| missing.clone())
        ),
        format!(
            "{}: {}",
            t!("İçindekiler girdisi", "Bookmarks"),
            report.outline_entries
        ),
    ];
    if report.sample_page > 0 {
        out.push(format!(
            "{}:\n  {sample}",
            t!(
                "Örnek metin (sayfa {})",
                "Sample text (page {})",
                report.sample_page
            )
        ));
    }
    out.join("\n")
}
