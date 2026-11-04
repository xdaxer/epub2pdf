# epub2pdf

[![CI](https://github.com/xdaxer/epub2pdf/actions/workflows/ci.yml/badge.svg)](https://github.com/xdaxer/epub2pdf/actions/workflows/ci.yml)

[Türkçe](#türkçe) · [English](#english)

---

## Türkçe

**epub2pdf**, EPUB kitaplarını okunaklı PDF'e çeviren bir komut satırı aracıdır.
Tek bir çalıştırılabilir dosyadır ve ek bir kurulum gerektirmez. Yazı tipi programın
içine gömülüdür, bu yüzden PDF her bilgisayarda aynı görünür.

- Cep kitabı boyunda (A5), iki yana yaslı, **Türkçe heceleme** ile dizilmiş metin
- Kapak tam sayfa, haritalar ve resimler sayfaya sığdırılmış
- Başlıklardan PDF içindekiler menüsü, sayfa numaraları, kitap içi bağlantılar
- Tek kitap, birden çok kitap ya da bütün bir klasör tek komutla
- Var olan PDF'in üzerine **asla** yazmaz
- Türkçe ve İngilizce arayüz

### Kurulum

[Releases](https://github.com/xdaxer/epub2pdf/releases) sayfasından sisteminize uygun
dosyayı indirin:

- **Linux:** `epub2pdf-…-linux-x86_64.tar.gz` dosyasını açın ve `epub2pdf`'i `PATH` üzerindeki
  bir klasöre (örneğin `~/.local/bin`) kopyalayın.
- **Windows:** `epub2pdf-…-windows-x86_64.zip` dosyasını açın ve `epub2pdf.exe`'yi `PATH`
  üzerindeki bir klasöre koyun ya da bulunduğu klasörde komut isteminden çalıştırın. Kaynaktan derlemek için
[Rust](https://rustup.rs) 1.92 veya üstü gerekir:

```bash
cargo install --path .
```

### Kullanım

```bash
epub2pdf kitap.epub                     # kitap.pdf, EPUB'ın yanına yazılır
epub2pdf *.epub                         # birden çok kitap
epub2pdf Kitaplar/                      # klasördeki tüm EPUB'lar
epub2pdf kitap.epub -o "Başka Ad.pdf"   # çıktıya başka ad ver
epub2pdf kitap.epub --boyut A4 --punto 12 --kenar 20
epub2pdf kitap.epub --onizleme onizleme/   # kapak, bir metin sayfası ve son sayfa PNG

epub2pdf bilgi kitap.epub               # künye: başlık, yazar, dil, bölüm sayısı...
epub2pdf kontrol kitap.pdf              # PDF'i denetle: sayfa, künye, içindekiler, örnek metin
```

| Seçenek | Kısa ad | Varsayılan |
|---|---|---|
| `--size` | `--boyut` | `A5` (A4, A5, A6, B5, Letter) |
| `--font` | | gömülü `DejaVu Serif`; yüklü bir yazı tipi adı ya da `.ttf/.otf` dosyası |
| `--font-size` | `--punto` | `10.5` |
| `--margins` | `--kenar` | `18,16,20` mm (üst, yan, alt) ya da tek değer |
| `--preview` | `--onizleme` | – |
| `--output`, `-o` | `--cikti` | EPUB'ın yanında aynı ad |
| `--config` | `--ayar` | kullanıcı ayar klasörü |

Çıkış kodları: `0` başarılı, `1` bir kitap dönüştürülemedi, `2` hatalı kullanım.
Toplu dönüştürmede PDF'i zaten olan kitaplar atlanır ve sonda özet yazılır.

### Ayar dosyası

Sık kullandığınız değerleri bir dosyaya yazabilirsiniz; komut satırı her zaman
dosyadaki değeri ezer.

- Linux: `~/.config/epub2pdf/config.toml`
- Windows: `%APPDATA%\epub2pdf\config.toml`

```toml
size = "A5"
font = "DejaVu Serif"
font_size = 10.5
margins = "18,16,20"
```

### Dil

Arayüz dili sistemin dil ayarından seçilir (Türkçe sistemde Türkçe, diğerlerinde
İngilizce). Zorlamak için: `EPUB2PDF_LANG=tr` ya da `EPUB2PDF_LANG=en`.

### Sınırlar

- Kitabın kendi CSS'i uygulanmaz; her kitap aynı sade düzende basılır.
- **DRM'li** (satın alınmış, kilitli) EPUB'lar açılamaz; araç bunu açıkça söyler.
- Kitap başlıkları `h1`/`h2` yerine biçimlendirilmiş paragraflarla yazılmışsa
  içindekiler menüsü boş kalabilir.

---

## English

**epub2pdf** is a command line tool that turns EPUB books into readable PDFs. It
is a single executable with no extra dependencies. The font is embedded, so the
PDF looks the same everywhere.

- Pocket-book (A5) layout, justified text, hyphenation in the book's language
- Full-page cover; maps and pictures scaled to fit the page
- PDF bookmarks from headings, page numbers, working internal links
- One book, many books or a whole folder in one command
- **Never** overwrites an existing PDF
- Turkish and English interface

### Install

Download the archive for your system from
[Releases](https://github.com/xdaxer/epub2pdf/releases):

- **Linux:** unpack `epub2pdf-…-linux-x86_64.tar.gz` and copy `epub2pdf` into a folder on
  your `PATH` (for example `~/.local/bin`).
- **Windows:** unpack `epub2pdf-…-windows-x86_64.zip` and put `epub2pdf.exe` into a folder
  on your `PATH`, or run it from its folder in a terminal. To build from source you need [Rust](https://rustup.rs) 1.92+:

```bash
cargo install --path .
```

### Usage

```bash
epub2pdf book.epub                      # writes book.pdf next to the EPUB
epub2pdf *.epub                         # several books
epub2pdf Books/                         # every EPUB in a folder
epub2pdf book.epub -o "Other name.pdf"
epub2pdf book.epub --size A4 --font-size 12 --margins 20
epub2pdf book.epub --preview preview/   # cover, a text page and the last page as PNG

epub2pdf info book.epub                 # metadata: title, author, language, chapters...
epub2pdf check book.pdf                 # inspect a PDF: pages, metadata, bookmarks, sample text
```

Every option also has a Turkish alias (`--boyut`, `--punto`, `--kenar`,
`--onizleme`, `--cikti`, `--ayar`; commands `kontrol` and `bilgi`). Run
`epub2pdf --help` for the full list.

Exit codes: `0` success, `1` a book could not be converted, `2` invalid usage.
In batch mode, books that already have a PDF are skipped and a summary is printed.

### Configuration

Defaults can be set in `config.toml` (command line values win):

- Linux: `~/.config/epub2pdf/config.toml`
- Windows: `%APPDATA%\epub2pdf\config.toml`

```toml
size = "A5"            # A4, A5, A6, B5, Letter
font = "DejaVu Serif"  # installed font name or path to a .ttf/.otf file
font_size = 10.5       # points
margins = "18,16,20"   # mm: top,sides,bottom (or one value for all)
```

### Language

The interface follows the system locale (Turkish for `tr*`, English otherwise).
Override it with `EPUB2PDF_LANG=tr` or `EPUB2PDF_LANG=en`.

### Limitations

- The book's own CSS is not applied; every book gets the same clean layout.
- EPUBs with **DRM** cannot be converted; the tool reports this clearly.
- If a book marks chapter titles with styled paragraphs instead of `h1`/`h2`,
  the bookmark list may be empty.

### How it works and development

See [docs/development.md](docs/development.md).

### License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at
your option. The embedded DejaVu fonts keep their own license, see
[assets/fonts/LICENSE](assets/fonts/LICENSE).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
