//! Tests of the individual modules against the synthetic sample books.

mod common;

use epub2pdf::config::{self, FileConfig, Overrides, Paper, Settings};
use epub2pdf::convert::{self, label_name};
use epub2pdf::epub::{self, EpubError};
use epub2pdf::{info, render};

fn sample() -> epub::Book {
    epub::read(&common::full_book()).expect("sample book reads")
}

// --- epub -----------------------------------------------------------------

#[test]
fn package_path_comes_from_container() {
    let book = sample();
    assert!(
        book.chapters
            .iter()
            .all(|c| c.path.starts_with("Kitap/Metin/"))
    );
}

#[test]
fn spine_order_is_kept_and_non_linear_items_are_skipped() {
    let paths: Vec<_> = sample().chapters.iter().map(|c| c.path.clone()).collect();
    assert_eq!(
        paths,
        [
            "Kitap/Metin/kapak.xhtml",
            "Kitap/Metin/bolum 1.xhtml",
            "Kitap/Metin/bolum2.xhtml",
            "Kitap/Metin/harita.xhtml",
        ]
    );
}

#[test]
fn metadata_is_read_exactly_once() {
    let m = sample().metadata;
    assert_eq!(m.title.as_deref(), Some(common::TITLE));
    assert_eq!(m.creators, [common::AUTHOR]);
    assert_eq!(m.language.as_deref(), Some("tr"));
    assert_eq!(m.publisher.as_deref(), Some("Örnek Yayınları"));
}

#[test]
fn broken_files_give_specific_errors() {
    assert!(matches!(
        epub::read(&common::broken_zip()),
        Err(EpubError::NotZip(_))
    ));
    assert!(matches!(
        epub::read(&common::no_container()),
        Err(EpubError::MissingContainer)
    ));
    assert!(matches!(
        epub::read(&common::drm_book()),
        Err(EpubError::Encrypted)
    ));
}

#[test]
fn font_obfuscation_is_not_drm() {
    assert!(epub::read(&common::obfuscated_font_book()).is_ok());
}

// --- convert --------------------------------------------------------------

#[test]
fn svg_cover_and_map_become_full_page_images() {
    let doc = convert::convert_book(&sample());
    let cover = doc.chapters[0].full_page_image.as_deref().expect("cover");
    let map = doc.chapters[3].full_page_image.as_deref().expect("map");
    let archive_path = |v: &str| {
        doc.images
            .iter()
            .find(|(virt, _)| virt == v)
            .map(|(_, a)| a.clone())
    };
    assert_eq!(
        archive_path(cover).as_deref(),
        Some("Kitap/Resimler/kapak.png")
    );
    assert_eq!(
        archive_path(map).as_deref(),
        Some("Kitap/Resimler/harita.png")
    );
    assert!(doc.chapters[1].full_page_image.is_none());
}

#[test]
fn text_is_emitted_as_strings_so_typst_syntax_stays_text() {
    let doc = convert::convert_book(&sample());
    let markup = &doc.chapters[1].markup;
    assert!(markup.contains(r##"#heading(level: 1)[#"Birinci" #"Bölüm"]"##));
    assert!(markup.contains(r##"#"#let""##));
    assert!(markup.contains(r##"#"1." #"madde""##));
    assert!(
        !markup.contains("#let x"),
        "raw Typst code leaked into markup"
    );
}

#[test]
fn links_point_to_labels_and_external_urls() {
    let doc = convert::convert_book(&sample());
    let one = &doc.chapters[1].markup;
    let two = &doc.chapters[2].markup;
    let note = label_name(2, Some("not3"));
    assert!(one.contains(&format!("#xref(\"{note}\")")));
    assert!(one.contains(&format!("#xref(\"{}\")", label_name(2, None))));
    assert!(one.contains("#link(\"https://example.org/\")"));
    assert!(
        !one.contains("yok"),
        "link to a missing file must become text"
    );
    assert!(two.contains(&format!("#metadata(none)<{note}>")));
    // Back link from chapter 2 to the heading id of chapter 1 (percent-encoded file name).
    assert!(two.contains(&format!("#xref(\"{}\")", label_name(1, Some("b1")))));
    assert!(one.contains(&format!("#metadata(none)<{}>", label_name(1, Some("b1")))));
}

#[test]
fn structure_is_preserved() {
    let doc = convert::convert_book(&sample());
    let two = &doc.chapters[2].markup;
    assert!(two.contains("#heading(level: 2)["));
    assert!(two.contains("#heading(level: 3, outlined: false)["));
    assert!(two.contains("#list(["));
    assert!(two.contains("#enum(["));
    assert!(two.contains("#quote(block: true)["));
    assert!(two.contains("#table(columns: 2,"));
    assert!(two.contains("#raw(block: true,"));
    assert!(two.contains("#align(center)["));
}

#[test]
fn self_closing_anchor_does_not_swallow_text() {
    let doc = convert::convert_book(&sample());
    let one = &doc.chapters[1].markup;
    // `<a id="sayfa5"/>` must not turn the following text into link content.
    assert!(one.contains(r##"#"Bağlantı:""##));
    assert!(!one.contains(r##"#xref("c1-sayfa5")"##));
}

#[test]
fn document_metadata_is_written_once() {
    let book = sample();
    let doc = convert::convert_book(&book);
    let source = render::build_source(&book, &doc, &Settings::default(), true);
    assert!(source.starts_with(
        "#set document(title: \"Deneme Kitabı: Şiir ve Ağaç\", author: (\"İpek Güneş\",))"
    ));
    assert!(source.contains("lang: \"tr\""));
    assert!(source.contains("paper: \"a4\""));
    assert!(source.contains("inside: 40mm, outside: 25mm"));
    assert!(source.contains("leading: 0.998em"), "1.5 line spacing");
}

// --- config ---------------------------------------------------------------

#[test]
fn defaults_are_a4_thesis_layout() {
    let s = Settings::default();
    assert_eq!(s.paper, Paper::A4);
    assert_eq!(s.font, "Times New Roman");
    assert_eq!(s.font_size, 12.0);
    assert_eq!(s.line_spacing, 1.5);
    let m = s.margins;
    assert_eq!(
        (m.top, m.inside, m.outside, m.bottom),
        (25.0, 40.0, 25.0, 25.0)
    );
}

#[test]
fn margins_take_one_three_or_four_values() {
    let m = config::parse_margins("20").unwrap();
    assert_eq!(
        (m.top, m.inside, m.outside, m.bottom),
        (20.0, 20.0, 20.0, 20.0)
    );
    let m = config::parse_margins("18,16,20").unwrap();
    assert_eq!(
        (m.top, m.inside, m.outside, m.bottom),
        (18.0, 16.0, 16.0, 20.0)
    );
    let m = config::parse_margins("25, 40, 25mm, 30").unwrap();
    assert_eq!(
        (m.top, m.inside, m.outside, m.bottom),
        (25.0, 40.0, 25.0, 30.0)
    );
    assert_eq!(m.to_string(), "25,40,25,30");
    assert_eq!(
        config::parse_margins("18,16,20").unwrap().to_string(),
        "18,16,20"
    );
    assert!(config::parse_margins("1,2,3,4,5").is_none());
}

#[test]
fn set_writes_validated_values_and_default_removes_them() {
    use config::Key;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub").join("config.toml");
    let s = config::set_values(
        &path,
        &[
            (Key::parse("punto").unwrap(), "10,5".into()),
            (Key::parse("satır_aralığı").unwrap(), "2".into()),
            (Key::parse("margins").unwrap(), "20".into()),
        ],
    )
    .unwrap();
    assert_eq!((s.font_size, s.line_spacing), (10.5, 2.0));
    let file = config::read_file(&path).unwrap();
    assert_eq!(file.font_size, Some(10.5));
    assert_eq!(file.margins.as_deref(), Some("20"));

    // An invalid value leaves the file untouched.
    let before = std::fs::read_to_string(&path).unwrap();
    assert!(config::set_values(&path, &[(Key::Size, "A3".into())]).is_err());
    assert!(config::set_values(&path, &[(Key::FontSize, "büyük".into())]).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);

    let s = config::set_values(&path, &[(Key::FontSize, "varsayılan".into())]).unwrap();
    assert_eq!(s.font_size, 12.0);
    assert_eq!(config::read_file(&path).unwrap().font_size, None);
    assert!(Key::parse("bilinmeyen").is_none());
}

#[test]
fn command_line_overrides_config_file() {
    let file = FileConfig {
        size: Some("A4".into()),
        font_size: Some(12.0),
        margins: Some("10".into()),
        ..Default::default()
    };
    let overrides = Overrides {
        font_size: Some(9.0),
        ..Default::default()
    };
    let s = config::combine(&file, &overrides).unwrap();
    assert_eq!(s.paper, Paper::A4);
    assert_eq!(s.font_size, 9.0);
    assert_eq!(s.margins.inside, 10.0);
}

#[test]
fn invalid_settings_are_rejected() {
    let bad = |o: Overrides| config::combine(&FileConfig::default(), &o).is_err();
    assert!(bad(Overrides {
        size: Some("A3".into()),
        ..Default::default()
    }));
    assert!(bad(Overrides {
        font_size: Some(2.0),
        ..Default::default()
    }));
    assert!(bad(Overrides {
        margins: Some("10,20".into()),
        ..Default::default()
    }));
    assert!(bad(Overrides {
        line_spacing: Some(5.0),
        ..Default::default()
    }));
    assert!(bad(Overrides {
        margins: Some("99".into()),
        ..Default::default()
    }));
}

#[test]
fn config_file_is_read_and_unknown_keys_fail() {
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("good.toml");
    std::fs::write(&good, "size = \"B5\"\nmargins = \"12, 14, 16\"\n").unwrap();
    let s = config::load(Some(&good), &Overrides::default()).unwrap();
    assert_eq!(s.paper, Paper::B5);
    assert_eq!(s.margins.bottom, 16.0);

    let unknown = dir.path().join("unknown.toml");
    std::fs::write(&unknown, "boyut = \"A4\"\n").unwrap();
    assert!(config::load(Some(&unknown), &Overrides::default()).is_err());

    let missing = dir.path().join("missing.toml");
    assert!(config::load(Some(&missing), &Overrides::default()).is_err());
}

// --- info -----------------------------------------------------------------

#[test]
fn info_summarises_the_book() {
    let i = info::summarize(&sample());
    assert_eq!(i.title.as_deref(), Some(common::TITLE));
    assert_eq!(i.authors, [common::AUTHOR]);
    assert_eq!(i.chapters, 4);
    assert_eq!(i.images, 2);
}
