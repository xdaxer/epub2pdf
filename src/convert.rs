//! Converting XHTML chapters into Typst markup.
//!
//! The book's own CSS is not interpreted; every book gets the same clean
//! layout from [`crate::render`]. What this module preserves is structure:
//! headings, paragraphs, emphasis, lists, quotes, tables, images and links.
//!
//! Text is emitted word by word as Typst string literals (`#"word"`), so no
//! character in a book can be mistaken for Typst syntax. Spaces between words
//! are plain markup spaces, which keeps Typst's line breaking and
//! hyphenation intact.

use std::collections::{HashMap, HashSet};

use scraper::{ElementRef, Html, Node};

use crate::epub::{Book, resolve, split_fragment};

/// Image formats Typst can decode.
const IMAGE_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "svg", "webp"];

/// A whole book as Typst markup.
#[derive(Debug, Default)]
pub struct Document {
    pub chapters: Vec<ChapterMarkup>,
    /// `(virtual path used in the markup, path inside the archive)`.
    pub images: Vec<(String, String)>,
}

/// One chapter as Typst markup.
#[derive(Debug, Default)]
pub struct ChapterMarkup {
    /// Label marking the start of the chapter (target of links to the file).
    pub label: String,
    pub markup: String,
    /// Set when the chapter consists of a single image and nothing else,
    /// such as a cover or a map. Holds the virtual image path.
    pub full_page_image: Option<String>,
}

/// Converts every chapter of `book`.
pub fn convert_book(book: &Book) -> Document {
    let parsed: Vec<Html> = book
        .chapters
        .iter()
        .map(|c| Html::parse_document(&expand_self_closing(&c.xhtml)))
        .collect();
    let chapter_index: HashMap<&str, usize> = book
        .chapters
        .iter()
        .enumerate()
        .map(|(i, c)| (c.path.as_str(), i))
        .collect();
    let targets = link_targets(book, &parsed, &chapter_index);

    let mut images = ImageTable::default();
    let mut chapters = Vec::new();
    for (index, (chapter, html)) in book.chapters.iter().zip(&parsed).enumerate() {
        let mut converter = Converter {
            book,
            chapter_path: &chapter.path,
            chapter: index,
            chapter_index: &chapter_index,
            targets: &targets,
            images: &mut images,
            emitted_labels: HashSet::new(),
            pending_anchor: String::new(),
        };
        chapters.push(converter.chapter(html));
    }
    Document {
        chapters,
        images: images.entries,
    }
}

/// Label of a chapter (`id == None`) or of an element inside it.
///
/// Labels only contain ASCII letters, digits, `-` and `_` so they are valid
/// in Typst's `<label>` syntax; other characters are hex-escaped.
pub fn label_name(chapter: usize, id: Option<&str>) -> String {
    let mut name = format!("c{chapter}");
    if let Some(id) = id {
        name.push('-');
        for c in id.chars() {
            if c.is_ascii_alphanumeric() || c == '-' {
                name.push(c);
            } else {
                name.push_str(&format!("_{:x}_", c as u32));
            }
        }
    }
    name
}

/// Escapes text for use inside a Typst string literal.
pub fn typst_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' | '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Rewrites XML self-closing tags such as `<a id="x"/>` or `<p/>` into an
/// explicit start and end tag. An HTML parser would otherwise read them as
/// unclosed start tags and swallow the text that follows.
pub fn expand_self_closing(xhtml: &str) -> String {
    const VOID: [&str; 14] = [
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ];
    let bytes = xhtml.as_bytes();
    let mut out = String::with_capacity(xhtml.len() + 64);
    let mut i = 0;
    let mut copied = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        // Skip comments, CDATA, doctype and processing instructions.
        if xhtml[i..].starts_with("<!--") {
            i = xhtml[i..].find("-->").map_or(bytes.len(), |p| i + p + 3);
            continue;
        }
        if xhtml[i..].starts_with("<!") || xhtml[i..].starts_with("<?") {
            i = xhtml[i..].find('>').map_or(bytes.len(), |p| i + p + 1);
            continue;
        }
        let name_end = xhtml[i + 1..]
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .map_or(bytes.len(), |p| i + 1 + p);
        let name = &xhtml[i + 1..name_end];
        // Find the closing '>' while respecting quoted attribute values.
        let mut j = name_end;
        let mut quote = None;
        while j < bytes.len() {
            let b = bytes[j];
            match quote {
                Some(q) if b == q => quote = None,
                Some(_) => {}
                None if b == b'"' || b == b'\'' => quote = Some(b),
                None if b == b'>' => break,
                None => {}
            }
            j += 1;
        }
        if j >= bytes.len() {
            break;
        }
        let local = name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase();
        if !name.is_empty()
            && !name.starts_with('/')
            && bytes[j - 1] == b'/'
            && !VOID.contains(&local.as_str())
        {
            out.push_str(&xhtml[copied..j - 1]);
            out.push_str("></");
            out.push_str(name);
            out.push('>');
            copied = j + 1;
        }
        i = j + 1;
    }
    out.push_str(&xhtml[copied..]);
    out
}

/// Collects every `(chapter, id)` that some internal link points to, so that
/// anchors are only emitted where they are needed.
fn link_targets(
    book: &Book,
    parsed: &[Html],
    chapter_index: &HashMap<&str, usize>,
) -> HashSet<String> {
    let selector = scraper::Selector::parse("a[href]").expect("valid selector");
    let mut targets = HashSet::new();
    for (chapter, html) in book.chapters.iter().zip(parsed) {
        for a in html.select(&selector) {
            let href = a.value().attr("href").unwrap_or_default();
            if is_external(href) {
                continue;
            }
            let (_, fragment) = split_fragment(href);
            let path = resolve(&chapter.path, href);
            if let (Some(&target), Some(id)) = (chapter_index.get(path.as_str()), fragment) {
                targets.insert(label_name(target, Some(id)));
            }
        }
    }
    targets
}

fn is_external(href: &str) -> bool {
    let lower = href.to_ascii_lowercase();
    ["http:", "https:", "mailto:", "ftp:", "tel:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}

/// Maps archive paths of images to short virtual paths for Typst.
#[derive(Default)]
struct ImageTable {
    entries: Vec<(String, String)>,
    by_archive_path: HashMap<String, String>,
}

impl ImageTable {
    fn register(&mut self, archive_path: &str) -> String {
        if let Some(virtual_path) = self.by_archive_path.get(archive_path) {
            return virtual_path.clone();
        }
        let ext = extension(archive_path);
        let virtual_path = format!("/img/{}.{ext}", self.entries.len() + 1);
        self.entries
            .push((virtual_path.clone(), archive_path.to_string()));
        self.by_archive_path
            .insert(archive_path.to_string(), virtual_path.clone());
        virtual_path
    }
}

fn extension(path: &str) -> String {
    path.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

const SKIPPED: [&str; 8] = [
    "head", "script", "style", "title", "meta", "link", "noscript", "template",
];

const BLOCKS: [&str; 30] = [
    "p",
    "div",
    "section",
    "article",
    "aside",
    "header",
    "footer",
    "main",
    "nav",
    "body",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "blockquote",
    "table",
    "hr",
    "pre",
    "figure",
    "figcaption",
    "center",
    "dl",
    "dt",
    "dd",
    "address",
];

struct Converter<'a> {
    book: &'a Book,
    chapter_path: &'a str,
    chapter: usize,
    chapter_index: &'a HashMap<&'a str, usize>,
    targets: &'a HashSet<String>,
    images: &'a mut ImageTable,
    emitted_labels: HashSet<String>,
    /// Anchors waiting to be attached to the next emitted content.
    pending_anchor: String,
}

/// Inline content being collected for one paragraph.
#[derive(Default)]
struct Inline {
    markup: String,
    /// True once something visible (a word, an image, a break) was added.
    visible: bool,
}

impl Inline {
    fn push_text(&mut self, text: &str) {
        let mut word = String::new();
        for c in text.chars() {
            if c.is_whitespace() && c != '\u{a0}' {
                self.flush_word(&mut word);
                if !self.markup.ends_with(' ') {
                    self.markup.push(' ');
                }
            } else {
                word.push(c);
            }
        }
        self.flush_word(&mut word);
    }

    fn flush_word(&mut self, word: &mut String) {
        if !word.is_empty() {
            self.markup.push('#');
            self.markup.push_str(&typst_string(word));
            self.visible = true;
            word.clear();
        }
    }

    fn push_markup(&mut self, markup: &str, visible: bool) {
        self.markup.push_str(markup);
        self.visible |= visible;
    }
}

impl Converter<'_> {
    fn chapter(&mut self, html: &Html) -> ChapterMarkup {
        let label = label_name(self.chapter, None);
        let body = html
            .select(&scraper::Selector::parse("body").expect("valid selector"))
            .next()
            .unwrap_or_else(|| html.root_element());

        if let Some(image) = self.single_image(body) {
            return ChapterMarkup {
                label,
                markup: String::new(),
                full_page_image: Some(image),
            };
        }

        let mut blocks = Vec::new();
        self.blocks(body, &mut blocks);
        if !self.pending_anchor.is_empty() {
            blocks.push(std::mem::take(&mut self.pending_anchor));
        }
        ChapterMarkup {
            label,
            markup: blocks.join("\n\n"),
            full_page_image: None,
        }
    }

    /// Returns the image of a chapter that holds exactly one image and no text.
    fn single_image(&mut self, body: ElementRef) -> Option<String> {
        let has_text = body.text().any(|t| t.chars().any(|c| !c.is_whitespace()));
        if has_text {
            return None;
        }
        let sources: Vec<String> = body
            .descendants()
            .filter_map(ElementRef::wrap)
            .filter_map(|e| match e.value().name() {
                "img" => e.value().attr("src").map(str::to_string),
                "image" => svg_href(e),
                _ => None,
            })
            .collect();
        match sources.as_slice() {
            [src] => self.image_path(src),
            _ => None,
        }
    }

    /// Converts the children of `parent` as a sequence of blocks.
    fn blocks(&mut self, parent: ElementRef, out: &mut Vec<String>) {
        let mut inline = Inline::default();
        for child in parent.children() {
            match child.value() {
                Node::Text(text) => inline.push_text(text),
                Node::Element(_) => {
                    let element = ElementRef::wrap(child).expect("element node");
                    let tag = element.value().name().to_ascii_lowercase();
                    if SKIPPED.contains(&tag.as_str()) {
                        continue;
                    }
                    if BLOCKS.contains(&tag.as_str()) {
                        self.flush(&mut inline, out);
                        self.block(element, &tag, out);
                    } else {
                        self.inline_element(element, &tag, &mut inline);
                    }
                }
                _ => {}
            }
        }
        self.flush(&mut inline, out);
    }

    /// Ends the current paragraph.
    fn flush(&mut self, inline: &mut Inline, out: &mut Vec<String>) {
        let taken = std::mem::take(inline);
        if taken.visible {
            let anchor = std::mem::take(&mut self.pending_anchor);
            out.push(format!("{anchor}{}", taken.markup.trim()));
        }
    }

    /// Pushes a finished block, attaching any pending anchor to it.
    fn push_block(&mut self, out: &mut Vec<String>, block: String) {
        if block.is_empty() {
            return;
        }
        let anchor = std::mem::take(&mut self.pending_anchor);
        out.push(format!("{anchor}{block}"));
    }

    /// Converts the children of `parent` into one markup string of blocks.
    fn nested_blocks(&mut self, parent: ElementRef) -> String {
        let mut inner = Vec::new();
        self.blocks(parent, &mut inner);
        inner.join("\n\n")
    }

    fn block(&mut self, element: ElementRef, tag: &str, out: &mut Vec<String>) {
        self.anchor_for(element);
        match tag {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = tag[1..].parse::<u8>().unwrap_or(1);
                let content = self.inline_children(element);
                if content.visible {
                    let outlined = if level >= 3 { ", outlined: false" } else { "" };
                    let block = format!(
                        "#heading(level: {level}{outlined})[{}]",
                        content.markup.trim()
                    );
                    self.push_block(out, block);
                }
            }
            "ul" | "ol" => {
                let mut items = Vec::new();
                for li in element.children().filter_map(ElementRef::wrap) {
                    if li.value().name() == "li" {
                        self.anchor_for(li);
                        let anchor = std::mem::take(&mut self.pending_anchor);
                        items.push(format!("[{anchor}{}]", self.nested_blocks(li)));
                    }
                }
                if !items.is_empty() {
                    let func = if tag == "ul" { "list" } else { "enum" };
                    self.push_block(out, format!("#{func}({})", items.join(", ")));
                }
            }
            "blockquote" => {
                let inner = self.nested_blocks(element);
                if !inner.is_empty() {
                    self.push_block(out, format!("#quote(block: true)[{inner}]"));
                }
            }
            "pre" => {
                let text: String = element.text().collect();
                if !text.trim().is_empty() {
                    let block = format!("#raw(block: true, {})", raw_string(&text));
                    self.push_block(out, block);
                }
            }
            "hr" => self.push_block(
                out,
                "#align(center, line(length: 30%, stroke: 0.5pt))".to_string(),
            ),
            "table" => {
                if let Some(table) = self.table(element) {
                    self.push_block(out, table);
                }
            }
            "figcaption" => {
                let inner = self.nested_blocks(element);
                if !inner.is_empty() {
                    let block = format!("#align(center, text(size: 0.9em)[{inner}])");
                    self.push_block(out, block);
                }
            }
            "dt" => {
                let content = self.inline_children(element);
                if content.visible {
                    self.push_block(out, format!("#strong[{}]", content.markup.trim()));
                }
            }
            "dd" => {
                let inner = self.nested_blocks(element);
                if !inner.is_empty() {
                    self.push_block(out, format!("#pad(left: 1.5em)[{inner}]"));
                }
            }
            _ => {
                if is_centered(element) || tag == "center" {
                    let inner = self.nested_blocks(element);
                    if !inner.is_empty() {
                        self.push_block(out, format!("#align(center)[{inner}]"));
                    }
                } else {
                    self.blocks(element, out);
                }
            }
        }
    }

    fn table(&mut self, table: ElementRef) -> Option<String> {
        let rows: Vec<ElementRef> = table
            .descendants()
            .filter_map(ElementRef::wrap)
            .filter(|e| e.value().name() == "tr" && belongs_to(*e, table))
            .collect();
        let mut grid: Vec<Vec<String>> = Vec::new();
        for row in rows {
            let mut cells = Vec::new();
            for cell in row.children().filter_map(ElementRef::wrap) {
                let name = cell.value().name();
                if name == "td" || name == "th" {
                    let inner = self.nested_blocks(cell);
                    cells.push(if name == "th" {
                        format!("[#strong[{inner}]]")
                    } else {
                        format!("[{inner}]")
                    });
                }
            }
            if !cells.is_empty() {
                grid.push(cells);
            }
        }
        let columns = grid.iter().map(Vec::len).max()?;
        let mut cells = Vec::new();
        for mut row in grid {
            row.resize(columns, "[]".to_string());
            cells.extend(row);
        }
        Some(format!(
            "#table(columns: {columns}, stroke: 0.4pt, inset: 4pt, {})",
            cells.join(", ")
        ))
    }

    /// Converts the children of an element into inline markup.
    fn inline_children(&mut self, element: ElementRef) -> Inline {
        let mut inline = Inline::default();
        for child in element.children() {
            match child.value() {
                Node::Text(text) => inline.push_text(text),
                Node::Element(_) => {
                    let el = ElementRef::wrap(child).expect("element node");
                    let tag = el.value().name().to_ascii_lowercase();
                    if !SKIPPED.contains(&tag.as_str()) {
                        if BLOCKS.contains(&tag.as_str()) {
                            // A block inside inline context (e.g. <div> in a
                            // heading): keep its text on the same line.
                            let nested = self.inline_children(el);
                            inline.push_markup(&nested.markup, nested.visible);
                            inline.push_markup(" ", false);
                        } else {
                            self.inline_element(el, &tag, &mut inline);
                        }
                    }
                }
                _ => {}
            }
        }
        inline
    }

    fn inline_element(&mut self, element: ElementRef, tag: &str, inline: &mut Inline) {
        self.anchor_for(element);
        let anchor = std::mem::take(&mut self.pending_anchor);
        inline.push_markup(&anchor, false);

        let wrap = |this: &mut Self, inline: &mut Inline, func: &str| {
            let inner = this.inline_children(element);
            if inner.visible {
                inline.push_markup(&format!("#{func}[{}]", inner.markup), true);
            } else {
                inline.push_markup(&inner.markup, false);
            }
        };
        match tag {
            "em" | "i" | "cite" | "dfn" | "var" => wrap(self, inline, "emph"),
            "strong" | "b" => wrap(self, inline, "strong"),
            "sup" => wrap(self, inline, "super"),
            "sub" => wrap(self, inline, "sub"),
            "u" | "ins" => wrap(self, inline, "underline"),
            "s" | "strike" | "del" => wrap(self, inline, "strike"),
            "small" => wrap(self, inline, "text(size: 0.85em)"),
            "code" | "kbd" | "samp" | "tt" => {
                let text: String = element.text().collect();
                if !text.trim().is_empty() {
                    inline.push_markup(&format!("#raw({})", raw_string(&text)), true);
                }
            }
            "br" => inline.push_markup("#linebreak()", true),
            "img" => {
                if let Some(path) = element.value().attr("src").and_then(|s| self.image_path(s)) {
                    inline.push_markup(&format!("#epubimg({})", typst_string(&path)), true);
                }
            }
            "svg" => {
                let href = element
                    .descendants()
                    .filter_map(ElementRef::wrap)
                    .find(|e| e.value().name() == "image")
                    .and_then(svg_href);
                if let Some(path) = href.and_then(|h| self.image_path(&h)) {
                    inline.push_markup(&format!("#epubimg({})", typst_string(&path)), true);
                }
            }
            "a" => self.link(element, inline),
            _ => {
                let inner = self.inline_children(element);
                inline.push_markup(&inner.markup, inner.visible);
            }
        }
    }

    fn link(&mut self, element: ElementRef, inline: &mut Inline) {
        let inner = self.inline_children(element);
        let Some(href) = element.value().attr("href") else {
            inline.push_markup(&inner.markup, inner.visible);
            return;
        };
        if !inner.visible {
            inline.push_markup(&inner.markup, false);
            return;
        }
        let body = &inner.markup;
        if is_external(href) {
            inline.push_markup(&format!("#link({})[{body}]", typst_string(href)), true);
            return;
        }
        let (_, fragment) = split_fragment(href);
        let path = resolve(self.chapter_path, href);
        match self.chapter_index.get(path.as_str()) {
            Some(&target) => {
                let label = label_name(target, fragment.filter(|f| !f.is_empty()));
                inline.push_markup(&format!("#xref({})[{body}]", typst_string(&label)), true);
            }
            None => inline.push_markup(body, true),
        }
    }

    /// Queues an anchor if `element` has an id that a link points to.
    fn anchor_for(&mut self, element: ElementRef) {
        let Some(id) = element.value().id() else {
            return;
        };
        let label = label_name(self.chapter, Some(id));
        if self.targets.contains(&label) && self.emitted_labels.insert(label.clone()) {
            self.pending_anchor
                .push_str(&format!("#metadata(none)<{label}>"));
        }
    }

    /// Registers an image referenced from the current chapter, returning its
    /// virtual path, or `None` when the file is missing or not supported.
    fn image_path(&mut self, src: &str) -> Option<String> {
        if is_external(src) || src.starts_with("data:") {
            return None;
        }
        let archive_path = resolve(self.chapter_path, src);
        if !IMAGE_EXTENSIONS.contains(&extension(&archive_path).as_str()) {
            return None;
        }
        self.book.file(&archive_path)?;
        Some(self.images.register(&archive_path))
    }
}

/// The `href` / `xlink:href` of an SVG `<image>` element.
fn svg_href(element: ElementRef) -> Option<String> {
    element
        .value()
        .attrs()
        .find(|(name, _)| *name == "href" || name.ends_with(":href"))
        .map(|(_, value)| value.to_string())
}

fn is_centered(element: ElementRef) -> bool {
    let style = element
        .value()
        .attr("style")
        .unwrap_or_default()
        .replace(' ', "")
        .to_ascii_lowercase();
    if style.contains("text-align:center") {
        return true;
    }
    element.value().classes().any(|c| {
        let c = c.to_ascii_lowercase();
        c == "center" || c == "centered" || c == "centre" || c == "ortala"
    })
}

/// True when the nearest `<table>` ancestor of `element` is `table`
/// (rows of nested tables belong to the inner table).
fn belongs_to(element: ElementRef, table: ElementRef) -> bool {
    element
        .ancestors()
        .filter_map(ElementRef::wrap)
        .find(|e| e.value().name() == "table")
        .is_some_and(|e| e.id() == table.id())
}

/// A Typst string for `raw`, keeping newlines.
fn raw_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("    "),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_escaped() {
        assert_eq!(typst_string(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(typst_string("x\ny"), "\"x y\"");
    }

    #[test]
    fn self_closing_tags_are_expanded() {
        assert_eq!(
            expand_self_closing(r#"<p><a id="x"/>Metin</p><br/><img src="a.png" />"#),
            r#"<p><a id="x"></a>Metin</p><br/><img src="a.png" />"#
        );
        assert_eq!(
            expand_self_closing(r#"<p title="a/>b"/>"#),
            r#"<p title="a/>b"></p>"#
        );
        assert_eq!(expand_self_closing("<!-- <p/> -->x"), "<!-- <p/> -->x");
    }

    #[test]
    fn labels_are_typst_safe() {
        assert_eq!(label_name(3, None), "c3");
        assert_eq!(label_name(3, Some("not-1")), "c3-not-1");
        assert_eq!(label_name(0, Some("a.b")), "c0-a_2e_b");
    }

    #[test]
    fn text_becomes_string_words() {
        let mut inline = Inline::default();
        inline.push_text("  Bir  #iki\n= üç ");
        assert_eq!(inline.markup, r##" #"Bir" #"#iki" #"=" #"üç" "##);
        assert!(inline.visible);
    }
}
