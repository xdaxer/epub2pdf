//! Reading EPUB archives.
//!
//! An EPUB is a zip archive. `META-INF/container.xml` points to the package
//! document (OPF), which lists the metadata, every file of the book (the
//! manifest) and the reading order (the spine). Folder names such as `OEBPS`
//! differ between publishers, so nothing here relies on them.

use std::collections::HashMap;
use std::fmt;
use std::io::{Cursor, Read};
use std::path::Path;

use crate::t;

const DC_NS: &str = "http://purl.org/dc/elements/1.1/";

/// Font obfuscation algorithms. Files "encrypted" with these are fonts that
/// were only mangled to prevent extraction; the text itself is readable.
const FONT_OBFUSCATION: [&str; 2] = [
    "http://www.idpf.org/2008/embedding",
    "http://ns.adobe.com/pdf/enc#RC",
];

/// A book loaded fully into memory.
#[derive(Debug)]
pub struct Book {
    pub metadata: Metadata,
    /// Chapters in reading order (non-linear spine items are left out).
    pub chapters: Vec<Chapter>,
    /// Number of images listed in the manifest.
    pub image_count: usize,
    files: HashMap<String, Vec<u8>>,
}

/// Bibliographic data from the package document.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Metadata {
    pub title: Option<String>,
    pub creators: Vec<String>,
    pub language: Option<String>,
    pub publisher: Option<String>,
    pub date: Option<String>,
    pub identifier: Option<String>,
}

/// One XHTML document of the spine.
#[derive(Debug, Clone)]
pub struct Chapter {
    /// Path of the document inside the archive, e.g. `OEBPS/Text/ch1.xhtml`.
    pub path: String,
    pub xhtml: String,
}

impl Book {
    /// Returns the bytes of a file inside the archive.
    ///
    /// Falls back to a case-insensitive match because some books reference
    /// `Images/Cover.JPG` as `images/cover.jpg`.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        if let Some(data) = self.files.get(path) {
            return Some(data);
        }
        let lower = path.to_lowercase();
        self.files
            .iter()
            .find(|(name, _)| name.to_lowercase() == lower)
            .map(|(_, data)| data.as_slice())
    }
}

/// Why a file could not be read as an EPUB.
#[derive(Debug)]
pub enum EpubError {
    Io(std::io::Error),
    /// The file is not a zip archive (or is truncated).
    NotZip(String),
    /// `META-INF/container.xml` is missing.
    MissingContainer,
    /// `container.xml` does not name a package document.
    NoRootfile,
    /// The package document named by `container.xml` does not exist.
    MissingPackage(String),
    /// An XML file could not be parsed.
    InvalidXml {
        file: String,
        message: String,
    },
    /// The book is protected with DRM.
    Encrypted,
    /// The spine contains no readable chapter.
    EmptySpine,
}

impl fmt::Display for EpubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            EpubError::Io(e) => t!("dosya okunamadı: {e}", "cannot read file: {e}"),
            EpubError::NotZip(e) => t!(
                "geçerli bir EPUB (zip) değil ya da dosya yarım inmiş ({e})",
                "not a valid EPUB (zip) file, or the download is incomplete ({e})"
            ),
            EpubError::MissingContainer => t!(
                "META-INF/container.xml yok; bu dosya bir EPUB değil",
                "META-INF/container.xml is missing; this is not an EPUB"
            ),
            EpubError::NoRootfile => t!(
                "container.xml kitabın paket dosyasını (OPF) göstermiyor",
                "container.xml does not point to a package document (OPF)"
            ),
            EpubError::MissingPackage(p) => t!(
                "paket dosyası bulunamadı: {p}",
                "package document not found: {p}"
            ),
            EpubError::InvalidXml { file, message } => t!(
                "{file} okunamadı (bozuk XML): {message}",
                "cannot parse {file} (invalid XML): {message}"
            ),
            EpubError::Encrypted => t!(
                "kitap DRM ile korunuyor (şifreli); bu araçla açılamaz",
                "the book is protected by DRM (encrypted) and cannot be converted"
            ),
            EpubError::EmptySpine => t!(
                "kitapta okunacak bölüm yok (spine boş)",
                "the book has no readable chapters (empty spine)"
            ),
        };
        f.write_str(&text)
    }
}

impl std::error::Error for EpubError {}

impl From<std::io::Error> for EpubError {
    fn from(e: std::io::Error) -> Self {
        EpubError::Io(e)
    }
}

/// Opens an EPUB file from disk.
pub fn open(path: &Path) -> Result<Book, EpubError> {
    read(&std::fs::read(path)?)
}

/// Reads an EPUB from memory.
pub fn read(data: &[u8]) -> Result<Book, EpubError> {
    let files = unzip(data)?;
    if is_encrypted(&files)? {
        return Err(EpubError::Encrypted);
    }

    let container = files
        .get("META-INF/container.xml")
        .ok_or(EpubError::MissingContainer)?;
    let container = parse_xml("META-INF/container.xml", container)?;
    let opf_path = container
        .descendants()
        .find(|n| n.has_tag_name("rootfile"))
        .and_then(|n| n.attribute("full-path"))
        .map(|p| normalize(&percent_decode(p)))
        .ok_or(EpubError::NoRootfile)?;
    let opf_bytes = files
        .get(&opf_path)
        .ok_or_else(|| EpubError::MissingPackage(opf_path.clone()))?;
    let opf = parse_xml(&opf_path, opf_bytes)?;

    let metadata = read_metadata(&opf);

    // Manifest: id -> (path in archive, media type).
    let mut manifest = HashMap::new();
    let mut image_count = 0;
    for item in opf.descendants().filter(|n| n.has_tag_name("item")) {
        let (Some(id), Some(href)) = (item.attribute("id"), item.attribute("href")) else {
            continue;
        };
        let media = item.attribute("media-type").unwrap_or_default();
        if media.starts_with("image/") {
            image_count += 1;
        }
        manifest.insert(
            id.to_string(),
            (resolve(&opf_path, href), media.to_string()),
        );
    }

    let mut chapters = Vec::new();
    for itemref in opf.descendants().filter(|n| n.has_tag_name("itemref")) {
        if itemref.attribute("linear") == Some("no") {
            continue;
        }
        let Some((path, media)) = itemref.attribute("idref").and_then(|id| manifest.get(id)) else {
            continue;
        };
        if !(media.contains("html") || path.ends_with("html") || path.ends_with(".htm")) {
            continue;
        }
        if let Some(bytes) = files.get(path) {
            chapters.push(Chapter {
                path: path.clone(),
                xhtml: decode_text(bytes),
            });
        }
    }
    if chapters.is_empty() {
        return Err(EpubError::EmptySpine);
    }

    Ok(Book {
        metadata,
        chapters,
        image_count,
        files,
    })
}

fn unzip(data: &[u8]) -> Result<HashMap<String, Vec<u8>>, EpubError> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(data)).map_err(|e| EpubError::NotZip(e.to_string()))?;
    let mut files = HashMap::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| EpubError::NotZip(e.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .name()
            .map_err(|e| EpubError::NotZip(e.to_string()))?
            .replace('\\', "/");
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut buf)
            .map_err(|e| EpubError::NotZip(e.to_string()))?;
        files.insert(name, buf);
    }
    Ok(files)
}

/// DRM check. `rights.xml` is Adobe ADEPT; `encryption.xml` is also used for
/// harmless font obfuscation, so only other algorithms count as DRM.
fn is_encrypted(files: &HashMap<String, Vec<u8>>) -> Result<bool, EpubError> {
    if files.contains_key("META-INF/rights.xml") {
        return Ok(true);
    }
    let Some(bytes) = files.get("META-INF/encryption.xml") else {
        return Ok(false);
    };
    let Ok(doc) = parse_xml("META-INF/encryption.xml", bytes) else {
        return Ok(true);
    };
    Ok(doc
        .descendants()
        .filter(|n| n.has_tag_name("EncryptionMethod"))
        .filter_map(|n| n.attribute("Algorithm"))
        .any(|alg| !FONT_OBFUSCATION.contains(&alg)))
}

fn read_metadata(opf: &roxmltree::Document) -> Metadata {
    let dc = |name: &str| -> Vec<String> {
        opf.descendants()
            .filter(|n| n.tag_name().namespace() == Some(DC_NS) && n.tag_name().name() == name)
            .filter_map(|n| {
                let text: String = n
                    .descendants()
                    .filter(|d| d.is_text())
                    .filter_map(|d| d.text())
                    .collect();
                let text = collapse_ws(&text);
                (!text.is_empty()).then_some(text)
            })
            .collect()
    };
    let first = |name: &str| dc(name).into_iter().next();
    Metadata {
        title: first("title"),
        creators: dc("creator"),
        language: first("language"),
        publisher: first("publisher"),
        date: first("date"),
        identifier: first("identifier"),
    }
}

fn parse_xml<'a>(file: &str, bytes: &'a [u8]) -> Result<roxmltree::Document<'a>, EpubError> {
    let invalid = |message: String| EpubError::InvalidXml {
        file: file.to_string(),
        message,
    };
    let text = std::str::from_utf8(bytes).map_err(|e| invalid(e.to_string()))?;
    let text = text.trim_start_matches('\u{feff}');
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    roxmltree::Document::parse_with_options(text, options).map_err(|e| invalid(e.to_string()))
}

/// Decodes a text file, dropping a UTF-8 byte order mark.
fn decode_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    text.trim_start_matches('\u{feff}').to_string()
}

fn collapse_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Splits `file.xhtml#anchor` into its path and fragment.
pub fn split_fragment(href: &str) -> (&str, Option<&str>) {
    match href.split_once('#') {
        Some((path, frag)) => (path, Some(frag)),
        None => (href, None),
    }
}

/// Resolves `href` (relative to the file `base`) to a path inside the archive.
///
/// The fragment and query are dropped and percent escapes are decoded:
/// `resolve("OEBPS/Text/c1.xhtml", "../Images/a%20b.jpg#x")` is
/// `OEBPS/Images/a b.jpg`.
pub fn resolve(base: &str, href: &str) -> String {
    let (path, _) = split_fragment(href);
    let path = path.split('?').next().unwrap_or_default();
    let path = percent_decode(path);
    if path.is_empty() {
        return base.to_string();
    }
    if let Some(absolute) = path.strip_prefix('/') {
        return normalize(absolute);
    }
    let dir = base.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    normalize(&format!("{dir}/{path}"))
}

/// Removes `.` and `..` segments and empty parts from a `/` path.
fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/")
}

/// Decodes `%XX` escapes; invalid escapes are kept as they are.
pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_paths() {
        assert_eq!(
            resolve("OEBPS/Text/c1.xhtml", "../Images/a%20b.jpg#x"),
            "OEBPS/Images/a b.jpg"
        );
        assert_eq!(resolve("c1.xhtml", "img/x.png"), "img/x.png");
        assert_eq!(resolve("A/B/c.xhtml", "./d.xhtml"), "A/B/d.xhtml");
        assert_eq!(resolve("A/c.xhtml", "#note"), "A/c.xhtml");
        assert_eq!(resolve("A/c.xhtml", "/root.png"), "root.png");
    }

    #[test]
    fn percent_decoding_handles_utf8_and_bad_escapes() {
        assert_eq!(percent_decode("%C3%A7ay"), "çay");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("a%zzb"), "a%zzb");
        assert_eq!(percent_decode("%4"), "%4");
    }
}
