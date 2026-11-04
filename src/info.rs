//! Summary of an EPUB's metadata (`epub2pdf info`).

use crate::epub::Book;
use crate::t;

/// What `epub2pdf info` shows about a book.
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub language: Option<String>,
    pub publisher: Option<String>,
    pub date: Option<String>,
    pub identifier: Option<String>,
    pub chapters: usize,
    pub images: usize,
}

pub fn summarize(book: &Book) -> Info {
    let m = &book.metadata;
    Info {
        title: m.title.clone(),
        authors: m.creators.clone(),
        language: m.language.clone(),
        publisher: m.publisher.clone(),
        date: m.date.clone(),
        identifier: m.identifier.clone(),
        chapters: book.chapters.len(),
        images: book.image_count,
    }
}

/// Formats the summary as aligned `label: value` lines.
pub fn format(info: &Info) -> String {
    let missing = t!("(yok)", "(none)");
    let or_missing = |v: &Option<String>| v.clone().unwrap_or_else(|| missing.clone());
    let authors = if info.authors.is_empty() {
        missing.clone()
    } else {
        info.authors.join(", ")
    };
    let rows = [
        (t!("Başlık", "Title"), or_missing(&info.title)),
        (t!("Yazar", "Author"), authors),
        (t!("Dil", "Language"), or_missing(&info.language)),
        (t!("Yayınevi", "Publisher"), or_missing(&info.publisher)),
        (t!("Tarih", "Date"), or_missing(&info.date)),
        (t!("Kimlik", "Identifier"), or_missing(&info.identifier)),
        (t!("Bölüm", "Chapters"), info.chapters.to_string()),
        (t!("Resim", "Images"), info.images.to_string()),
    ];
    let width = rows
        .iter()
        .map(|(l, _)| l.chars().count())
        .max()
        .unwrap_or(0);
    rows.iter()
        .map(|(label, value)| {
            let pad = width - label.chars().count();
            format!("{label}{}  {value}", " ".repeat(pad))
        })
        .collect::<Vec<_>>()
        .join("\n")
}
