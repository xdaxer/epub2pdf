//! Typesetting the converted book with Typst and writing the PDF.
//!
//! The Typst "world" is entirely in memory: one generated source file, the
//! book's images under `/img/..`, the embedded Liberation Serif and DejaVu
//! fonts and, unless one of those is requested, the system fonts.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use typst::diag::{FileError, FileResult, Warned};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::fonts::FontStore;
use typst_layout::PagedDocument;

use crate::config::{DEFAULT_FONT, FALLBACK_FONT, LAST_RESORT_FONT, Settings};
use crate::convert::{self, Document, typst_string};
use crate::epub::Book;
use crate::t;

/// Fonts compiled into the binary so the output looks the same everywhere.
/// Liberation Serif has the same metrics as Times New Roman, so a book set
/// with it breaks lines and pages exactly as it would with Times New Roman.
const EMBEDDED_FONTS: [&[u8]; 9] = [
    include_bytes!("../assets/fonts/LiberationSerif-Regular.ttf"),
    include_bytes!("../assets/fonts/LiberationSerif-Bold.ttf"),
    include_bytes!("../assets/fonts/LiberationSerif-Italic.ttf"),
    include_bytes!("../assets/fonts/LiberationSerif-BoldItalic.ttf"),
    include_bytes!("../assets/fonts/DejaVuSerif.ttf"),
    include_bytes!("../assets/fonts/DejaVuSerif-Bold.ttf"),
    include_bytes!("../assets/fonts/DejaVuSerif-Italic.ttf"),
    include_bytes!("../assets/fonts/DejaVuSerif-BoldItalic.ttf"),
    include_bytes!("../assets/fonts/DejaVuSansMono.ttf"),
];

/// Result of a successful conversion.
#[derive(Debug)]
pub struct Rendered {
    pub pdf: Vec<u8>,
    pub page_count: usize,
    /// Human readable notes, e.g. that images had to be left out.
    pub warnings: Vec<String>,
    /// PNG files written for `--preview`.
    pub previews: Vec<PathBuf>,
}

#[derive(Debug)]
pub enum RenderError {
    FontNotFound(String),
    Typst(Vec<String>),
    Pdf(Vec<String>),
    Preview(PathBuf, String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            RenderError::FontNotFound(name) => t!(
                "yazı tipi bulunamadı: '{name}' (sistemde yüklü bir yazı tipi adı ya da .ttf/.otf dosya yolu verin)",
                "font not found: '{name}' (give an installed font name or a .ttf/.otf file path)"
            ),
            RenderError::Typst(errors) => t!(
                "dizgi hatası: {}",
                "typesetting failed: {}",
                errors.join("; ")
            ),
            RenderError::Pdf(errors) => t!(
                "PDF yazılamadı: {}",
                "PDF export failed: {}",
                errors.join("; ")
            ),
            RenderError::Preview(path, e) => t!(
                "önizleme yazılamadı ({}): {e}",
                "cannot write preview ({}): {e}",
                path.display()
            ),
        };
        f.write_str(&text)
    }
}

impl std::error::Error for RenderError {}

/// Where and how to write page previews.
pub struct Preview<'a> {
    pub dir: &'a Path,
    /// File name prefix, usually the book's file stem.
    pub prefix: &'a str,
}

/// Converts and typesets `book`.
pub fn render(
    book: &Book,
    settings: &Settings,
    preview: Option<Preview>,
) -> Result<Rendered, RenderError> {
    let document = convert::convert_book(book);
    let (fonts, font_note) = load_fonts(&settings.font)?;
    let images = image_files(book, &document);

    // A font given as a file path is used under its family name.
    let mut settings = settings.clone();
    settings.font = font_family(&settings.font);
    let settings = &settings;

    let mut warnings: Vec<String> = font_note.into_iter().collect();
    let source = build_source(book, &document, settings, true);
    // Debugging aid: EPUB2PDF_DUMP_TYPST=file.typ writes the generated source.
    if let Some(path) = std::env::var_os("EPUB2PDF_DUMP_TYPST") {
        let _ = std::fs::write(path, &source);
    }
    let mut world = BookWorld::new(source, fonts, images);
    let doc = match compile(&world) {
        Ok(doc) => doc,
        Err(errors) if !document.images.is_empty() => {
            // A broken or unsupported image must not cost the whole book.
            warnings.push(t!(
                "resimler basılamadı, kitap resimsiz basıldı ({})",
                "images could not be rendered; the book was typeset without them ({})",
                errors.join("; ")
            ));
            world.set_source(build_source(book, &document, settings, false));
            compile(&world).map_err(RenderError::Typst)?
        }
        Err(errors) => return Err(RenderError::Typst(errors)),
    };

    let pdf = typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default())
        .map_err(|errors| RenderError::Pdf(messages(&errors)))?;
    let previews = match preview {
        Some(p) => write_previews(&doc, &p)?,
        None => Vec::new(),
    };
    Ok(Rendered {
        pdf,
        page_count: doc.pages().len(),
        warnings,
        previews,
    })
}

fn compile(world: &BookWorld) -> Result<PagedDocument, Vec<String>> {
    let Warned { output, .. } = typst::compile::<PagedDocument>(world);
    output.map_err(|errors| messages(&errors))
}

fn messages(diagnostics: &[typst::diag::SourceDiagnostic]) -> Vec<String> {
    diagnostics.iter().map(|d| d.message.to_string()).collect()
}

/// Builds the Typst source of the whole book.
///
/// With `images == false` every image is left out (used as a fallback when an
/// image cannot be decoded).
pub fn build_source(book: &Book, document: &Document, settings: &Settings, images: bool) -> String {
    let meta = &book.metadata;
    let title = meta.title.clone().unwrap_or_default();
    let lang = language_code(meta.language.as_deref());
    let m = &settings.margins;
    let fs = settings.font_size;

    let mut src = String::new();
    src.push_str(&format!("#set document(title: {}", typst_string(&title)));
    if !meta.creators.is_empty() {
        let authors: Vec<String> = meta.creators.iter().map(|a| typst_string(a)).collect();
        src.push_str(&format!(", author: ({},)", authors.join(", ")));
    }
    src.push_str(")\n");
    // Nothing may be printed outside the margins, page numbers included, so
    // the page number gets its own strip inside the bottom margin line: two
    // blank single-spaced lines above it, then the number itself.
    let footer_gap = 2.0 * SINGLE_LINE * fs;
    let footer_strip = footer_gap + fs;
    src.push_str(&format!(
        "#set page(paper: \"{}\", margin: (top: {}mm, bottom: {}mm + {footer_strip:.2}pt, \
         inside: {}mm, outside: {}mm), footer-descent: {footer_gap:.2}pt, \
         footer: context align(center, counter(page).display()))\n",
        settings.paper.typst_name(),
        m.top,
        m.bottom,
        m.inside,
        m.outside
    ));
    src.push_str(&format!(
        "#set text(font: ({}, {}, {}), size: {fs}pt, lang: \"{lang}\", hyphenate: true)\n",
        typst_string(&settings.font),
        typst_string(FALLBACK_FONT),
        typst_string(LAST_RESORT_FONT),
    ));
    let leading = leading_em(settings.line_spacing);
    let single = leading_em(1.0);
    // Chapter titles start 3.5 cm below the top edge of the page.
    let chapter_gap = (CHAPTER_TOP_MM - m.top).max(0.0);
    src.push_str(&format!(
        "#set par(justify: true, leading: {leading:.3}em, spacing: {leading:.3}em, \
         first-line-indent: 1.25cm)\n\
         #show quote: set par(leading: {single:.3}em, spacing: {single:.3}em)\n\
         #show table: set par(leading: {single:.3}em, spacing: {single:.3}em)\n\
         #show heading: set text(size: {fs}pt)\n\
         #show heading.where(level: 1): it => {{ v({chapter_gap}mm); it }}\n"
    ));
    src.push_str(TEMPLATE);
    if images {
        src.push_str(IMAGE_FUNCTION);
    } else {
        src.push_str("#let epubimg(path) = []\n");
    }

    for (index, chapter) in document.chapters.iter().enumerate() {
        let anchor = format!("#metadata(none)<{}>", chapter.label);
        match &chapter.full_page_image {
            Some(path) if index == 0 => {
                if images {
                    src.push_str(&format!(
                        "#page(margin: 0pt, footer: none)[{anchor}#image({}, width: 100%, height: 100%, fit: \"contain\")]\n\n",
                        typst_string(path)
                    ));
                } else {
                    src.push_str(&format!("{anchor}\n\n"));
                }
            }
            Some(path) => {
                src.push_str(&format!(
                    "#pagebreak(weak: true)\n{anchor}#epubimg({})\n\n",
                    typst_string(path)
                ));
            }
            None => {
                if index > 0 {
                    src.push_str("#pagebreak(weak: true)\n");
                }
                src.push_str(&anchor);
                src.push('\n');
                src.push_str(&chapter.markup);
                src.push_str("\n\n");
            }
        }
    }
    src
}

/// Height of one single-spaced line (ascender + descender) of Times New
/// Roman, in em. Word processors multiply this for 1.5 or double spacing.
const SINGLE_LINE: f64 = 1.107;
/// Cap height of Times New Roman in em; Typst measures lines from it.
const CAP_HEIGHT: f64 = 0.662;
/// Distance of a chapter title from the top edge of the page, in mm.
const CHAPTER_TOP_MM: f64 = 35.0;

/// Typst `par.leading` for a word processor style line spacing (1, 1.5, 2).
fn leading_em(line_spacing: f64) -> f64 {
    (line_spacing * SINGLE_LINE - CAP_HEIGHT).max(0.1)
}

/// Page and text styles shared by every book.
///
/// Following common thesis rules, all headings are bold at the body text
/// size (set in [`build_source`]), chapter (level 1) titles are centred, and paragraphs are separated
/// by a first-line indent instead of extra space.
const TEMPLATE: &str = r#"#show heading: set text(weight: "bold", hyphenate: false)
#show heading: set par(justify: false)
#show heading: set block(above: 1.5em, below: 1em)
#show heading.where(level: 1): set align(center)
#show heading.where(level: 1): set block(above: 1.5em, below: 2em)
#show raw: set text(font: ("DejaVu Sans Mono",), size: 0.9em)
#let xref(name, body) = context {
  let found = query(label(name))
  if found.len() == 1 { link(found.first().location(), body) } else { body }
}
"#;

/// Places an image centred and scaled down to fit the text area.
const IMAGE_FUNCTION: &str = r#"#let epubimg(path) = layout(size => {
  let img = image(path)
  let s = measure(img)
  let max-height = calc.min(size.height, 180mm)
  if s.width == 0pt or s.height == 0pt {
    align(center, img)
  } else {
    let scale = calc.min(1.0, size.width / s.width, max-height / s.height)
    align(center, image(path, width: s.width * scale))
  }
})
"#;

/// `tr`, `en-US`, `tr_TR` -> `tr`. Defaults to Turkish.
fn language_code(language: Option<&str>) -> String {
    let code: String = language
        .unwrap_or("tr")
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_lowercase();
    if (2..=3).contains(&code.len()) {
        code
    } else {
        "tr".to_string()
    }
}

/// Embedded fonts, plus either a font file or the system fonts when a font
/// other than the embedded ones is requested.
///
/// Times New Roman is not embedded (its license does not allow it); when it
/// is not installed the metric-compatible Liberation Serif is used and a note
/// is returned.
fn load_fonts(font: &str) -> Result<(FontStore, Option<String>), RenderError> {
    let mut store = FontStore::new();
    store.extend(EMBEDDED_FONTS.iter().flat_map(|data| {
        Font::iter(Bytes::new(*data)).map(|f| {
            let info = f.info().clone();
            (f, info)
        })
    }));
    if font.eq_ignore_ascii_case(FALLBACK_FONT) || font.eq_ignore_ascii_case(LAST_RESORT_FONT) {
        return Ok((store, None));
    }

    let path = Path::new(font);
    if path.is_file() {
        let data = std::fs::read(path).map_err(|_| RenderError::FontNotFound(font.to_string()))?;
        let faces: Vec<(Font, typst::text::FontInfo)> = Font::iter(Bytes::new(data))
            .map(|f| {
                let info = f.info().clone();
                (f, info)
            })
            .collect();
        if faces.is_empty() {
            return Err(RenderError::FontNotFound(font.to_string()));
        }
        store.extend(faces);
        return Ok((store, None));
    }

    store.extend(typst_kit::fonts::system());
    if store
        .book()
        .select_family(&font.to_lowercase())
        .next()
        .is_none()
    {
        if font.eq_ignore_ascii_case(DEFAULT_FONT) {
            let note = t!(
                "Times New Roman yüklü değil; ölçüleri aynı olan gömülü Liberation Serif kullanıldı",
                "Times New Roman is not installed; the metric-compatible embedded Liberation Serif was used"
            );
            return Ok((store, Some(note)));
        }
        return Err(RenderError::FontNotFound(font.to_string()));
    }
    Ok((store, None))
}

/// The family name to use in Typst for a `--font` value (a file path is
/// replaced by the family name inside the file).
pub fn font_family(font: &str) -> String {
    let path = Path::new(font);
    if path.is_file()
        && let Ok(data) = std::fs::read(path)
        && let Some(f) = Font::iter(Bytes::new(data)).next()
    {
        return f.info().family.clone();
    }
    font.to_string()
}

fn image_files(book: &Book, document: &Document) -> HashMap<String, Bytes> {
    document
        .images
        .iter()
        .filter_map(|(virtual_path, archive_path)| {
            let data = book.file(archive_path)?;
            Some((virtual_path.clone(), Bytes::new(data.to_vec())))
        })
        .collect()
}

fn write_previews(doc: &PagedDocument, preview: &Preview) -> Result<Vec<PathBuf>, RenderError> {
    let pages = doc.pages();
    if pages.is_empty() {
        return Ok(Vec::new());
    }
    let last = pages.len() - 1;
    let text_page = if pages.len() > 13 {
        12
    } else {
        pages.len() / 2
    };
    let mut indices = vec![0, text_page, last];
    indices.dedup();
    indices.sort_unstable();
    indices.dedup();

    std::fs::create_dir_all(preview.dir)
        .map_err(|e| RenderError::Preview(preview.dir.to_path_buf(), e.to_string()))?;
    let options = typst_render::RenderOptions {
        pixel_per_pt: 1.5.into(),
        ..Default::default()
    };
    let mut written = Vec::new();
    for index in indices {
        let pixmap = typst_render::render(&pages[index], &options);
        let path = preview
            .dir
            .join(format!("{}-page-{}.png", preview.prefix, index + 1));
        let png = pixmap
            .encode_png()
            .map_err(|e| RenderError::Preview(path.clone(), e.to_string()))?;
        std::fs::write(&path, png)
            .map_err(|e| RenderError::Preview(path.clone(), e.to_string()))?;
        written.push(path);
    }
    Ok(written)
}

/// An in-memory Typst world holding one book.
struct BookWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    main: FileId,
    source: Source,
    files: HashMap<String, Bytes>,
}

impl BookWorld {
    fn new(text: String, fonts: FontStore, files: HashMap<String, Bytes>) -> Self {
        let main = file_id("/main.typ");
        BookWorld {
            library: LazyHash::new(Library::default()),
            fonts,
            main,
            source: Source::new(main, text),
            files,
        }
    }

    fn set_source(&mut self, text: String) {
        self.source = Source::new(self.main, text);
    }
}

fn file_id(path: &str) -> FileId {
    let vpath = VirtualPath::new(path).expect("valid virtual path");
    FileId::new(RootedPath::new(VirtualRoot::Project, vpath))
}

impl World for BookWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main {
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound(id.vpath().get_with_slash().into()))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.vpath().get_with_slash();
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| FileError::NotFound(path.into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_codes_are_normalised() {
        assert_eq!(language_code(Some("en-US")), "en");
        assert_eq!(language_code(Some("tr_TR")), "tr");
        assert_eq!(language_code(Some("")), "tr");
        assert_eq!(language_code(None), "tr");
    }
}
