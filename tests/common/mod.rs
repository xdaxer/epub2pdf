//! Synthetic EPUB files for tests and comparisons.
//!
//! Every quirk the converter has to handle is built in on purpose:
//! non-standard folder names (the package path is only known from
//! `container.xml`), an SVG-wrapped cover, an image-only map chapter, links
//! between chapters, a non-linear spine item, Turkish metadata, characters
//! that are Typst syntax, XML self-closing tags, DRM and font obfuscation.

#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;

/// Text that must appear in the PDF exactly once in each place it is used.
pub const TURKISH_SAMPLE: &str = "Çağrı ışığı söndürdü; ğüşiöç İĞÜŞÖÇ.";
/// Text of a non-linear chapter; it must not appear in the PDF.
pub const HIDDEN_TEXT: &str = "GIZLIEKMETNI";
pub const TITLE: &str = "Deneme Kitabı: Şiir ve Ağaç";
pub const AUTHOR: &str = "İpek Güneş";

/// Encodes a solid colour RGB PNG (zlib with stored blocks, no compression).
pub fn solid_png(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend(data);
        out.extend(&body);
        out.extend(crc32(&body).to_be_bytes());
    }
    let row: Vec<u8> = std::iter::once(0) // filter: none
        .chain(std::iter::repeat_n(rgb, width as usize).flatten())
        .collect();
    let raw = row.repeat(height as usize);
    let mut zlib = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, block) in blocks.iter().enumerate() {
        zlib.push(u8::from(i + 1 == blocks.len()));
        let len = block.len() as u16;
        zlib.extend(len.to_le_bytes());
        zlib.extend((!len).to_le_bytes());
        zlib.extend(*block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + byte as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    zlib.extend(((b << 16) | a).to_be_bytes());

    let mut ihdr = Vec::new();
    ihdr.extend(width.to_be_bytes());
    ihdr.extend(height.to_be_bytes());
    ihdr.extend([8, 2, 0, 0, 0]); // 8-bit RGB
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &zlib);
    chunk(&mut png, b"IEND", &[]);
    png
}

const CONTAINER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="Kitap/paket.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;

const OPF: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="kimlik">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Deneme Kitabı: Şiir ve Ağaç</dc:title>
    <dc:creator>İpek Güneş</dc:creator>
    <dc:language>tr</dc:language>
    <dc:publisher>Örnek Yayınları</dc:publisher>
    <dc:date>2026-11-04</dc:date>
    <dc:identifier id="kimlik">urn:uuid:00000000-0000-4000-8000-000000000001</dc:identifier>
  </metadata>
  <manifest>
    <item id="kapak" href="Metin/kapak.xhtml" media-type="application/xhtml+xml"/>
    <item id="b1" href="Metin/bolum%201.xhtml" media-type="application/xhtml+xml"/>
    <item id="b2" href="Metin/bolum2.xhtml" media-type="application/xhtml+xml"/>
    <item id="ek" href="Metin/ek.xhtml" media-type="application/xhtml+xml"/>
    <item id="harita" href="Metin/harita.xhtml" media-type="application/xhtml+xml"/>
    <item id="stil" href="Stil/stil.css" media-type="text/css"/>
    <item id="kapak-resmi" href="Resimler/kapak.png" media-type="image/png" properties="cover-image"/>
    <item id="harita-resmi" href="Resimler/harita.png" media-type="image/png"/>
  </manifest>
  <spine>
    <itemref idref="kapak"/>
    <itemref idref="b1"/>
    <itemref idref="b2"/>
    <itemref idref="ek" linear="no"/>
    <itemref idref="harita"/>
  </spine>
</package>"#;

const COVER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:xlink="http://www.w3.org/1999/xlink">
<head><title>Kapak</title></head>
<body>
  <div>
    <svg xmlns="http://www.w3.org/2000/svg" version="1.1" viewBox="0 0 60 90">
      <image width="60" height="90" xlink:href="../Resimler/kapak.png"/>
    </svg>
  </div>
</body>
</html>"#;

const MAP: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Harita</title></head>
<body><div class="resim"><img src="../Resimler/harita.png" alt="Harita"/></div></body>
</html>"#;

const APPENDIX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Ek</title></head>
<body><p>GIZLIEKMETNI</p></body>
</html>"#;

fn chapter_one() -> String {
    let mut filler = String::new();
    for i in 1..=60 {
        filler.push_str(&format!(
            "<p>Dolgu paragrafı {i}: Uzun kış gecelerinde kuzeyden gelen haberler kale \
             duvarlarının ardında fısıltıyla konuşulur, çocuklar ocak başında eski \
             masalları dinlerdi; heceleme ve iki yana yaslama bu satırlarda sınanır.</p>\n"
        ));
    }
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Birinci Bölüm</title><link rel="stylesheet" href="../Stil/stil.css"/></head>
<body>
  <h1 id="b1">Birinci Bölüm</h1>
  <p>{TURKISH_SAMPLE}</p>
  <p>Özel karakterler: # * _ $ [ ] &lt; &gt; @ ~ = - + / \ " ' ` ve #let x = 1</p>
  <p>1. madde gibi başlayan satır listeye dönüşmemeli.</p>
  <p>= Bu da başlık olmamalı.</p>
  <p><em>italik</em>, <strong>kalın</strong>, <i>eğik</i> ve <b>koyu</b> yazı.<a id="sayfa5"/> Bağlantı:
     <a href="bolum2.xhtml#not3">üçüncü dipnot</a>, <a href="bolum2.xhtml">ikinci bölüm</a>,
     <a href="yok.xhtml#x">kırık bağlantı</a> ve <a href="https://example.org/">dış bağlantı</a>.</p>
  <p>Satır<br/>kırılımı.</p>
  {filler}
</body>
</html>"##
    )
}

const CHAPTER_TWO: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>İkinci Bölüm</title></head>
<body>
  <section>
    <h2>İkinci Bölüm</h2>
    <p class="center">Ortalanmış satır</p>
    <h3>Alt başlık</h3>
    <ul><li>Birinci madde</li><li>İkinci madde <em>vurgulu</em></li></ul>
    <ol><li>Bir</li><li>İki</li></ol>
    <blockquote><p>Alıntı paragrafı.</p></blockquote>
    <table>
      <tr><th>Ad</th><th>Değer</th></tr>
      <tr><td>Şehir</td><td>İstanbul</td></tr>
      <tr><td>Tek hücre</td></tr>
    </table>
    <pre>kod satırı 1
  kod "satırı" 2</pre>
    <p id="not3">3. Dipnot metni. <a href="bolum%201.xhtml#b1">Geri dön</a></p>
  </section>
</body>
</html>"##;

const CSS: &str = "p { text-indent: 1em; }\n@font-face { src: url(../Fontlar/yok.ttf); }\n";

/// All files of the full sample book, without the `mimetype` entry.
pub fn book_files() -> Vec<(String, Vec<u8>)> {
    vec![
        ("META-INF/container.xml".into(), CONTAINER.into()),
        ("Kitap/paket.opf".into(), OPF.into()),
        ("Kitap/Metin/kapak.xhtml".into(), COVER.into()),
        (
            "Kitap/Metin/bolum 1.xhtml".into(),
            chapter_one().into_bytes(),
        ),
        ("Kitap/Metin/bolum2.xhtml".into(), CHAPTER_TWO.into()),
        ("Kitap/Metin/ek.xhtml".into(), APPENDIX.into()),
        ("Kitap/Metin/harita.xhtml".into(), MAP.into()),
        ("Kitap/Stil/stil.css".into(), CSS.into()),
        (
            "Kitap/Resimler/kapak.png".into(),
            solid_png(60, 90, [180, 40, 40]),
        ),
        (
            "Kitap/Resimler/harita.png".into(),
            solid_png(120, 80, [40, 120, 60]),
        ),
    ]
}

/// Packs files into an EPUB (the `mimetype` entry comes first, uncompressed).
pub fn zip_epub(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut out);
        let stored =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflated =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("mimetype", stored).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();
        for (name, data) in files {
            zip.start_file(name.as_str(), deflated).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
    out.into_inner()
}

/// The full sample book.
pub fn full_book() -> Vec<u8> {
    zip_epub(&book_files())
}

/// A small book without a cover: three plain chapters.
pub fn plain_book() -> Vec<u8> {
    let opf = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Sade Kitap</dc:title><dc:creator>Deneme Yazar</dc:creator><dc:language>tr</dc:language>
  </metadata>
  <manifest>
    <item id="a" href="a.xhtml" media-type="application/xhtml+xml"/>
    <item id="b" href="b.xhtml" media-type="application/xhtml+xml"/>
    <item id="c" href="c.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="a"/><itemref idref="b"/><itemref idref="c"/></spine>
</package>"#;
    let container = CONTAINER.replace("Kitap/paket.opf", "OEBPS/content.opf");
    let mut files = vec![
        ("META-INF/container.xml".to_string(), container.into_bytes()),
        ("OEBPS/content.opf".to_string(), opf.as_bytes().to_vec()),
    ];
    for (id, title) in [("a", "Başlangıç"), ("b", "Gelişme"), ("c", "Sonuç")] {
        let mut body = format!("<h1>{title}</h1>");
        for i in 1..=25 {
            body.push_str(&format!(
                "<p>{title} bölümünün {i}. paragrafı: ağır ağır ilerleyen kervan, \
                 çölün ortasında ışıl ışıl parlayan bir vahaya ulaştı.</p>"
            ));
        }
        let xhtml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>{title}</title></head><body>{body}</body></html>"
        );
        files.push((format!("OEBPS/{id}.xhtml"), xhtml.into_bytes()));
    }
    zip_epub(&files)
}

/// Not a zip archive at all.
pub fn broken_zip() -> Vec<u8> {
    b"PK\x03\x04 bu bir zip degil".to_vec()
}

/// A zip without `META-INF/container.xml`.
pub fn no_container() -> Vec<u8> {
    let files: Vec<_> = book_files()
        .into_iter()
        .filter(|(name, _)| name != "META-INF/container.xml")
        .collect();
    zip_epub(&files)
}

fn with_encryption(algorithm: &str, target: &str) -> Vec<u8> {
    let mut files = book_files();
    files.push((
        "META-INF/encryption.xml".into(),
        format!(
            r#"<?xml version="1.0"?>
<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container"
            xmlns:enc="http://www.w3.org/2001/04/xmlenc#">
  <enc:EncryptedData>
    <enc:EncryptionMethod Algorithm="{algorithm}"/>
    <enc:CipherData><enc:CipherReference URI="{target}"/></enc:CipherData>
  </enc:EncryptedData>
</encryption>"#
        )
        .into_bytes(),
    ));
    zip_epub(&files)
}

/// Content encrypted with AES: real DRM.
pub fn drm_book() -> Vec<u8> {
    with_encryption(
        "http://www.w3.org/2001/04/xmlenc#aes128-cbc",
        "Kitap/Metin/bolum2.xhtml",
    )
}

/// Only a font is obfuscated: not DRM, the book must convert.
pub fn obfuscated_font_book() -> Vec<u8> {
    with_encryption(
        "http://www.idpf.org/2008/embedding",
        "Kitap/Fontlar/yok.ttf",
    )
}

/// Writes the sample books into `dir` and returns their paths.
pub fn write_samples(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let mut paths = Vec::new();
    for (name, data) in [
        ("ornek-kitap.epub", full_book()),
        ("sade-kitap.epub", plain_book()),
    ] {
        let path = dir.join(name);
        std::fs::write(&path, data)?;
        paths.push(path);
    }
    Ok(paths)
}
