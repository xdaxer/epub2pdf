# epub2pdf

[![CI](https://github.com/xdaxer/epub2pdf/actions/workflows/ci.yml/badge.svg)](https://github.com/xdaxer/epub2pdf/actions/workflows/ci.yml)

[Türkçe](#türkçe) · [English](#english)

---

## Türkçe

**epub2pdf**, EPUB kitaplarını okunaklı PDF'e çeviren bir komut satırı aracıdır.
Tek bir çalıştırılabilir dosyadır ve ek bir kurulum gerektirmez. Yazı tipi programın
içine gömülüdür, bu yüzden PDF her bilgisayarda aynı görünür.

- Varsayılan düzen klasik **tez düzenidir**: A4, Times New Roman 12 punto,
  1,5 satır aralığı, cilt tarafında 4 cm, diğer kenarlarda 2,5 cm boşluk
- İki yana yaslı, **Türkçe heceleme** ile dizilmiş metin
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
epub2pdf kitap.epub --boyut A5 --punto 11 --kenar 20 --satir-araligi 1
epub2pdf kitap.epub --onizleme onizleme/   # kapak, bir metin sayfası ve son sayfa PNG

epub2pdf bilgi kitap.epub               # künye: başlık, yazar, dil, bölüm sayısı...
epub2pdf kontrol kitap.pdf              # PDF'i denetle: sayfa, künye, içindekiler, örnek metin

epub2pdf ayarla                         # kayıtlı ayarları göster
epub2pdf ayarla punto 11 boyut A5       # ayarları kalıcı olarak değiştir
epub2pdf ayarla punto varsayilan        # bir ayarı varsayılana döndür
epub2pdf ayarla --sifirla               # tümünü varsayılanlara döndür
```

| Seçenek | Kısa ad | Varsayılan |
|---|---|---|
| `--size` | `--boyut` | `A4` (A4, A5, A6, B5, Letter) |
| `--font` | | `Times New Roman`; yüklü bir yazı tipi adı ya da `.ttf/.otf` dosyası |
| `--font-size` | `--punto` | `12` |
| `--line-spacing` | `--satir-araligi` | `1.5` (1 tek, 2 çift) |
| `--margins` | `--kenar` | `25,40,25,25` mm (üst, iç/cilt, dış, alt); `üst,yan,alt` ya da tek değer de olur |
| `--preview` | `--onizleme` | – |
| `--output`, `-o` | `--cikti` | EPUB'ın yanında aynı ad |
| `--config` | `--ayar` | kullanıcı ayar klasörü |

Çıkış kodları: `0` başarılı, `1` bir kitap dönüştürülemedi, `2` hatalı kullanım.
Toplu dönüştürmede PDF'i zaten olan kitaplar atlanır ve sonda özet yazılır.

### Ayar dosyası

Sık kullandığınız değerleri `epub2pdf ayarla` ile kaydedebilir ya da dosyaya
elle yazabilirsiniz; komut satırı her zaman dosyadaki değeri ezer.
`ayarla` dosyayı yeniden yazdığı için dosyadaki yorumlar silinir.

- Linux: `~/.config/epub2pdf/config.toml`
- Windows: `%APPDATA%\epub2pdf\config.toml`

```toml
size = "A4"
font = "Times New Roman"
font_size = 12.0
line_spacing = 1.5
margins = "25,40,25,25"
```

### Varsayılan düzen

Varsayılan ayarlar şunlardır:

- A4 kâğıt; sayfalar çift taraflı basılacağı için cilt tarafı (tek sayfalarda
  sol, çift sayfalarda sağ) 4 cm, diğer kenarlar 2,5 cm.
- Kenar boşluklarına hiçbir şey taşmaz; sayfa numarası da bu alanın içinde,
  metinden iki satır aşağıda ortalanır.
- Times New Roman 12 punto, 1,5 satır aralığı; alıntı ve tablolar tek aralıklı.
- Paragraflar arasında boşluk yerine ilk satır girintisi (1,25 cm).
- Bütün başlıklar metin boyunda ve kalın; bölüm başlıkları ortalı ve sayfanın
  üstünden 3,5 cm aşağıda.

Times New Roman'ın lisansı programa gömülmesine izin vermez. Sistemde yüklü
değilse (çoğu Linux'ta yüklü değildir), ölçüleri birebir aynı olan gömülü
**Liberation Serif** kullanılır ve bir uyarı yazılır; satır ve sayfa kırılımları
değişmez.

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

- Default layout is a classic **thesis layout**: A4, Times New Roman 12 pt,
  1.5 line spacing, 4 cm on the binding side and 2.5 cm on the others
- Justified text, hyphenation in the book's language
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
epub2pdf book.epub --size A5 --font-size 11 --margins 20 --line-spacing 1
epub2pdf book.epub --preview preview/   # cover, a text page and the last page as PNG

epub2pdf info book.epub                 # metadata: title, author, language, chapters...
epub2pdf check book.pdf                 # inspect a PDF: pages, metadata, bookmarks, sample text

epub2pdf set                            # show the saved settings
epub2pdf set font-size 11 size A5       # change settings permanently
epub2pdf set font-size default          # back to the default for one setting
epub2pdf set --reset                    # back to all defaults
```

Every option also has a Turkish alias (`--boyut`, `--punto`, `--kenar`,
`--satir-araligi`, `--onizleme`, `--cikti`, `--ayar`; commands `kontrol`,
`bilgi` and `ayarla`). Run
`epub2pdf --help` for the full list.

Exit codes: `0` success, `1` a book could not be converted, `2` invalid usage.
In batch mode, books that already have a PDF are skipped and a summary is printed.

### Configuration

Defaults can be saved with `epub2pdf set` or written to `config.toml` by hand
(command line values win). `set` rewrites the file, so comments in it are lost.

- Linux: `~/.config/epub2pdf/config.toml`
- Windows: `%APPDATA%\epub2pdf\config.toml`

```toml
size = "A4"               # A4, A5, A6, B5, Letter
font = "Times New Roman"  # installed font name or path to a .ttf/.otf file
font_size = 12.0          # points
line_spacing = 1.5        # 1 single, 2 double
margins = "25,40,25,25"   # mm: top,inside,outside,bottom (or top,sides,bottom, or one value)
```

### Default layout

The defaults are A4 paper printed two-sided, a 4 cm binding margin (left on odd pages, right on
even pages) and 2.5 cm elsewhere, with nothing, page numbers included, outside
the margins; Times New Roman 12 pt at 1.5 spacing (quotes and tables single
spaced); first-line indents instead of space between paragraphs; all headings
bold at text size, chapter titles centred 3.5 cm from the top of the page.

Times New Roman cannot be embedded for licensing reasons. When it is not
installed, the embedded, metric-compatible **Liberation Serif** is used (line
and page breaks are identical) and a warning is printed.

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
your option. The embedded Liberation and DejaVu fonts keep their own licenses, see
[assets/fonts/LICENSE](assets/fonts/LICENSE).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
