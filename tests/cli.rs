//! End-to-end tests of the `epub2pdf` binary.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use lopdf::Document;

/// Runs the binary in the given language with an isolated config file.
fn run(lang: &str, dir: &Path, args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_epub2pdf"))
        .args(args)
        .env("EPUB2PDF_LANG", lang)
        .env("EPUB2PDF_CONFIG", dir.join("no-config.toml"))
        .output()
        .expect("binary runs")
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn write(dir: &Path, name: &str, data: Vec<u8>) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, data).unwrap();
    path
}

fn all_text(pdf: &Path) -> String {
    let doc = Document::load(pdf).unwrap();
    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    doc.extract_text(&pages).unwrap()
}

#[test]
fn converts_a_book_next_to_the_epub() {
    let dir = tempfile::tempdir().unwrap();
    let epub = write(dir.path(), "kitap.epub", common::full_book());
    let out = run("tr", dir.path(), &[epub.as_os_str()]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert!(text(&out).contains("TAMAM"));

    let pdf = dir.path().join("kitap.pdf");
    let report = epub2pdf::check::inspect(&pdf).unwrap();
    assert!(report.pages >= 4, "pages: {}", report.pages);
    assert_eq!(report.title.as_deref(), Some(common::TITLE));
    assert_eq!(report.author.as_deref(), Some(common::AUTHOR));
    assert_eq!(report.outline_entries, 2, "h1 and h2 only");

    let content = all_text(&pdf);
    assert!(content.contains("ğüşiöç"), "Turkish letters lost");
    assert!(content.contains("İĞÜŞÖÇ"), "Turkish capitals lost");
    assert!(
        content.contains("#let x = 1"),
        "Typst syntax must stay text"
    );
    assert!(
        !content.contains(common::HIDDEN_TEXT),
        "non-linear chapter was printed"
    );
    assert!(!dir.path().join("kitap.pdf.part").exists());
}

#[test]
fn cover_page_is_an_image_without_page_number() {
    let dir = tempfile::tempdir().unwrap();
    let epub = write(dir.path(), "kitap.epub", common::full_book());
    assert_eq!(
        run("en", dir.path(), &[epub.as_os_str()]).status.code(),
        Some(0)
    );
    let doc = Document::load(dir.path().join("kitap.pdf")).unwrap();
    let pages = doc.get_pages();
    let first = pages[&1];
    assert_eq!(doc.get_page_images(first).unwrap().len(), 1, "cover image");
    assert!(
        doc.extract_text(&[1]).unwrap().trim().is_empty(),
        "cover has no text"
    );
    assert!(
        doc.extract_text(&[2]).unwrap().contains('2'),
        "page number on page 2"
    );
}

#[test]
fn existing_pdf_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let epub = write(dir.path(), "kitap.epub", common::full_book());
    let pdf = write(dir.path(), "kitap.pdf", b"eski".to_vec());
    let out = run("tr", dir.path(), &[epub.as_os_str()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out).contains("zaten var"));
    assert_eq!(std::fs::read(pdf).unwrap(), b"eski");
}

#[test]
fn folder_batch_skips_existing_and_reports_summary() {
    let dir = tempfile::tempdir().unwrap();
    let books = dir.path().join("kitaplar");
    std::fs::create_dir(&books).unwrap();
    write(&books, "a.epub", common::plain_book());
    write(&books, "b.EPUB", common::full_book());
    write(&books, "a.pdf", b"eski".to_vec());
    write(&books, "not.txt", b"x".to_vec());

    let out = run("tr", dir.path(), &[books.as_os_str()]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert!(
        text(&out).contains("Özet: 1 dönüştürüldü, 1 atlandı, 0 hatalı"),
        "{}",
        text(&out)
    );
    assert!(books.join("b.pdf").exists());
    assert_eq!(std::fs::read(books.join("a.pdf")).unwrap(), b"eski");
}

#[test]
fn batch_continues_after_a_broken_book_and_fails_at_the_end() {
    let dir = tempfile::tempdir().unwrap();
    let bad = write(dir.path(), "bozuk.epub", common::broken_zip());
    let good = write(dir.path(), "iyi.epub", common::plain_book());
    let out = run("en", dir.path(), &[bad.as_os_str(), good.as_os_str()]);
    assert_eq!(out.status.code(), Some(1));
    let t = text(&out);
    assert!(t.contains("not a valid EPUB"), "{t}");
    assert!(
        t.contains("Summary: 1 converted, 0 skipped, 1 failed"),
        "{t}"
    );
    assert!(dir.path().join("iyi.pdf").exists());
}

#[test]
fn drm_and_missing_files_are_reported_in_turkish() {
    let dir = tempfile::tempdir().unwrap();
    let drm = write(dir.path(), "drm.epub", common::drm_book());
    let out = run("tr", dir.path(), &[drm.as_os_str()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out).contains("DRM"), "{}", text(&out));
    assert!(!text(&out).contains("panicked"));

    let missing = dir.path().join("yok.epub");
    let out = run("tr", dir.path(), &[missing.as_os_str()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out).contains("dosya bulunamadı"));
}

#[test]
fn output_name_only_with_a_single_book() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(dir.path(), "a.epub", common::plain_book());
    let b = write(dir.path(), "b.epub", common::plain_book());
    let out = run(
        "en",
        dir.path(),
        &[
            a.as_os_str(),
            b.as_os_str(),
            "-o".as_ref(),
            "x.pdf".as_ref(),
        ],
    );
    assert_eq!(out.status.code(), Some(2));

    let custom = dir.path().join("özel ad.pdf");
    let out = run(
        "en",
        dir.path(),
        &[a.as_os_str(), "--cikti".as_ref(), custom.as_os_str()],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert!(custom.exists());
}

#[test]
fn layout_options_change_the_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let epub = write(dir.path(), "a.epub", common::plain_book());
    let a4 = dir.path().join("a4.pdf");
    let out = run(
        "tr",
        dir.path(),
        &[
            epub.as_os_str(),
            "-o".as_ref(),
            a4.as_os_str(),
            "--boyut".as_ref(),
            "A4".as_ref(),
            "--punto".as_ref(),
            "12".as_ref(),
            "--kenar".as_ref(),
            "25".as_ref(),
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let doc = Document::load(&a4).unwrap();
    let page = doc.get_pages()[&1];
    let media = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"MediaBox")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    let width = media[2].as_float().unwrap();
    assert!((width - 595.0).abs() < 2.0, "A4 width, got {width}");

    let out = run(
        "tr",
        dir.path(),
        &[epub.as_os_str(), "--size".as_ref(), "A3".as_ref()],
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out).contains("bilinmeyen sayfa boyutu"));
}

#[test]
fn preview_writes_three_pngs() {
    let dir = tempfile::tempdir().unwrap();
    let epub = write(dir.path(), "kitap.epub", common::full_book());
    let previews = dir.path().join("onizleme");
    let out = run(
        "tr",
        dir.path(),
        &[
            epub.as_os_str(),
            "--onizleme".as_ref(),
            previews.as_os_str(),
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let pngs: Vec<_> = std::fs::read_dir(&previews).unwrap().flatten().collect();
    assert_eq!(pngs.len(), 3);
    for png in pngs {
        assert!(std::fs::read(png.path()).unwrap().starts_with(b"\x89PNG"));
    }
}

#[test]
fn check_and_info_work_with_both_names() {
    let dir = tempfile::tempdir().unwrap();
    let epub = write(dir.path(), "kitap.epub", common::full_book());
    assert_eq!(
        run("tr", dir.path(), &[epub.as_os_str()]).status.code(),
        Some(0)
    );
    let pdf = dir.path().join("kitap.pdf");

    for name in ["check", "kontrol"] {
        let out = run("tr", dir.path(), &[name.as_ref(), pdf.as_os_str()]);
        assert_eq!(out.status.code(), Some(0));
        assert!(
            text(&out).contains("İçindekiler girdisi: 2"),
            "{}",
            text(&out)
        );
    }
    for name in ["info", "bilgi"] {
        let out = run("en", dir.path(), &[name.as_ref(), epub.as_os_str()]);
        assert_eq!(out.status.code(), Some(0));
        let t = text(&out);
        assert!(
            t.contains(common::TITLE) && t.contains("Chapters    4"),
            "{t}"
        );
    }
    let out = run("en", dir.path(), &["check".as_ref(), epub.as_os_str()]);
    assert_eq!(out.status.code(), Some(1), "an EPUB is not a PDF");
}

#[test]
fn help_is_available_in_both_languages() {
    let dir = tempfile::tempdir().unwrap();
    let tr = run("tr", dir.path(), &["--help".as_ref()]);
    let en = run("en", dir.path(), &["--help".as_ref()]);
    assert_eq!(tr.status.code(), Some(0));
    assert!(text(&tr).contains("Kullanım:") && text(&tr).contains("--onizleme"));
    assert!(text(&en).contains("Usage:") && text(&en).contains("--preview"));
    assert!(!text(&tr).contains("Usage:"));
}
