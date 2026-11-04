# Development

## Toolchain

epub2pdf needs Rust 1.92 or newer.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
rustup component add clippy rustfmt
```

## Build and test

```bash
cargo build --release   # target/release/epub2pdf
cargo test
cargo clippy --all-targets
cargo fmt --check
```

The tests never touch your real configuration: they set `EPUB2PDF_CONFIG` to a
file that does not exist and `EPUB2PDF_LANG` to `tr` or `en`.

Sample books for manual testing:

```bash
cargo run --example sample_epub -- samples/
cargo run -- samples/ --preview samples/preview
```

Set `EPUB2PDF_DUMP_TYPST=book.typ` to write the generated Typst source of a
conversion to a file; this is the quickest way to debug layout problems.

## How a conversion works

An EPUB is a zip archive of XHTML pages, images and a few XML files.

1. **Locate the package** (`src/epub.rs`). `META-INF/container.xml` names the
   package document (OPF). Folder names such as `OEBPS/` vary between
   publishers, so nothing else is assumed. The OPF holds the metadata
   (`dc:title`, `dc:creator`, `dc:language`, ...), the manifest (every file)
   and the spine (reading order). Spine items marked `linear="no"` are skipped.
   `META-INF/rights.xml`, or an `encryption.xml` that uses anything other than
   font obfuscation, means the book has DRM and cannot be converted.
2. **Convert chapters to Typst** (`src/convert.rs`). Each XHTML page is parsed
   with an HTML parser after XML self-closing tags (`<a id="x"/>`) are
   expanded. Structure is kept (headings, paragraphs, emphasis, lists, quotes,
   tables, preformatted text, images, links); the book's CSS is not applied.
   Text is written word by word as Typst string literals, so characters such
   as `#`, `*`, `=` or `1.` at a line start can never become Typst syntax.
   - An SVG-wrapped image (a common cover format) becomes a plain image.
   - Relative image paths are resolved inside the archive and served to Typst
     under short virtual paths (`/img/1.jpg`).
   - Links such as `chapter5.xhtml#note3` become document-internal links; a
     link whose target does not exist stays plain text.
   - A chapter that consists of a single image and no text is a full-page
     image. If it is the first chapter it is printed as a cover without margin
     and page number.
3. **Typeset** (`src/render.rs`). The generated source sets the page size,
   margins, page numbers, the embedded DejaVu Serif font, justified text with
   hyphenation in the book's language, a page break before each chapter and
   PDF bookmarks from `h1`/`h2`. Typst compiles it in memory and `typst-pdf`
   writes the PDF with the title and author. If an image cannot be decoded,
   the book is typeset again without images and a warning is printed.
4. **Write** (`src/cli.rs`). The PDF is written to `NAME.pdf.part` and renamed,
   so an interrupted run never leaves a broken PDF. Existing PDFs are never
   overwritten.

`epub2pdf check` (`src/check.rs`) reads any PDF with `lopdf` and reports the
page count, title, author, number of bookmarks and a text sample.

## Interface language

All user-facing text is written with the `t!` macro (`src/i18n.rs`), which
keeps the Turkish and English strings side by side:

```rust
eprintln!("{}", t!("dosya bulunamadı: {}", "file not found: {}", path.display()));
```

The language comes from `EPUB2PDF_LANG` (`tr`/`en`), otherwise from the system
locale (`tr*` selects Turkish, anything else English). Code, comments and
developer documentation are in English.
