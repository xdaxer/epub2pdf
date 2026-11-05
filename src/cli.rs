//! The `epub2pdf` command line.
//!
//! ```text
//! epub2pdf [OPTIONS] <EPUB_OR_DIR>...   convert (PDF is written next to the EPUB)
//! epub2pdf check <PDF>                  inspect a PDF      (alias: kontrol)
//! epub2pdf info <EPUB>                  show EPUB metadata (alias: bilgi)
//! epub2pdf set [KEY VALUE]...           show or change saved settings (alias: ayarla)
//! ```
//!
//! Exit codes: 0 success, 1 a book could not be converted or inspected,
//! 2 invalid usage or settings.

use std::ffi::OsString;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::config::{self, ConfigError, FileConfig, Key, Overrides, Paper, Settings};
use crate::i18n::Lang;
use crate::render::{self, Preview};
use crate::{check, epub, info, t};

pub const EXIT_OK: i32 = 0;
pub const EXIT_FAILED: i32 = 1;
pub const EXIT_USAGE: i32 = 2;

/// Runs the command line and returns the process exit code.
pub fn run<I, T>(args: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = match command().try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            use clap::error::ErrorKind;
            return match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    let _ = e.print();
                    EXIT_OK
                }
                _ => {
                    let _ = e.print();
                    eprintln!(
                        "{}",
                        t!("Yardım için: epub2pdf --help", "For help: epub2pdf --help")
                    );
                    EXIT_USAGE
                }
            };
        }
    };
    match matches.subcommand() {
        Some(("check", sub)) => run_check(sub),
        Some(("info", sub)) => run_info(sub),
        Some(("set", sub)) => run_set(sub),
        _ => run_convert(&matches),
    }
}

/// Builds the clap command with texts in the interface language.
pub fn command() -> Command {
    let help = |c: Command| {
        c.disable_help_flag(true).arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::Help)
                .help(t!("Bu yardımı göster", "Print this help")),
        )
    };
    let path_arg = |name: &'static str, value: String, text: String| {
        Arg::new(name)
            .value_name(value)
            .required(true)
            .value_parser(value_parser!(PathBuf))
            .help(text)
    };

    let check = help(
        Command::new("check")
            .visible_alias("kontrol")
            .about(t!(
                "PDF'i denetle: sayfa, künye, içindekiler, örnek metin",
                "Inspect a PDF: pages, metadata, bookmarks, sample text"
            ))
            .help_template(sub_template())
            .arg(path_arg(
                "pdf",
                "PDF".into(),
                t!("Denetlenecek PDF", "PDF to inspect"),
            )),
    );
    let info = help(
        Command::new("info")
            .visible_alias("bilgi")
            .about(t!("EPUB'ın künyesini göster", "Show an EPUB's metadata"))
            .help_template(sub_template())
            .arg(path_arg(
                "epub",
                "EPUB".into(),
                t!("İncelenecek EPUB", "EPUB to inspect"),
            )),
    );

    let config_arg = || {
        Arg::new("config")
            .long("config")
            .visible_alias("ayar")
            .value_name(t!("DOSYA", "FILE"))
            .value_parser(value_parser!(PathBuf))
            .help(t!(
                "Ayar dosyası (varsayılan: kullanıcı ayar klasöründe epub2pdf/config.toml)",
                "Config file (default: epub2pdf/config.toml in the user config directory)"
            ))
    };
    let set = help(
        Command::new("set")
            .visible_alias("ayarla")
            .about(t!(
                "Kayıtlı ayarları göster ya da değiştir",
                "Show or change the saved settings"
            ))
            .after_help(t!(
                "Ayarlar: {}\n'varsayilan' değeri ayarı siler.\n\nÖrnekler:\n  epub2pdf set                       ayarları göster\n  epub2pdf set punto 11 boyut A5\n  epub2pdf set kenar 25,40,25,25     üst, iç (cilt), dış, alt mm\n  epub2pdf set yazi-tipi \"DejaVu Serif\"\n  epub2pdf set punto varsayilan\n  epub2pdf set --sifirla             tümünü varsayılanlara döndür",
                "Settings: {}\nThe value 'default' removes a setting.\n\nExamples:\n  epub2pdf set                       show the settings\n  epub2pdf set font-size 11 size A5\n  epub2pdf set margins 25,40,25,25   top, inside (binding), outside, bottom mm\n  epub2pdf set font \"DejaVu Serif\"\n  epub2pdf set font-size default\n  epub2pdf set --reset               restore all defaults",
                Key::NAMES
            ))
            .help_template(sub_template())
            .arg(
                Arg::new("pairs")
                    .value_name(t!("AYAR DEĞER", "KEY VALUE"))
                    .num_args(0..)
                    .help(t!(
                        "Değiştirilecek ayar ve yeni değeri; birden çok çift verilebilir",
                        "Setting to change and its new value; several pairs may be given"
                    )),
            )
            .arg(
                Arg::new("reset")
                    .long("reset")
                    .visible_alias("sifirla")
                    .action(ArgAction::SetTrue)
                    .conflicts_with("pairs")
                    .help(t!(
                        "Ayar dosyasını sil, varsayılanlara dön",
                        "Delete the config file and go back to the defaults"
                    )),
            )
            .arg(config_arg()),
    );

    help(Command::new("epub2pdf"))
        .version(env!("CARGO_PKG_VERSION"))
        .about(t!(
            "EPUB kitaplarını okunaklı PDF'e çevirir. PDF, EPUB'ın yanına aynı adla yazılır.",
            "Converts EPUB books into readable PDFs, written next to each EPUB."
        ))
        .override_usage(t!(
            "epub2pdf [SEÇENEKLER] <EPUB_VEYA_KLASÖR>...\n       epub2pdf kontrol <PDF>\n       epub2pdf bilgi <EPUB>\n       epub2pdf ayarla [AYAR DEĞER]...",
            "epub2pdf [OPTIONS] <EPUB_OR_DIR>...\n       epub2pdf check <PDF>\n       epub2pdf info <EPUB>\n       epub2pdf set [KEY VALUE]..."
        ))
        .help_template(main_template())
        .disable_help_subcommand(true)
        .disable_version_flag(true)
        .args_conflicts_with_subcommands(true)
        .subcommand_negates_reqs(true)
        .arg(
            Arg::new("inputs")
                .value_name(t!("EPUB_VEYA_KLASÖR", "EPUB_OR_DIR"))
                .num_args(1..)
                .required(true)
                .value_parser(value_parser!(PathBuf))
                .help(t!(
                    "Dönüştürülecek EPUB dosyaları ya da EPUB içeren klasörler",
                    "EPUB files, or folders containing EPUB files, to convert"
                )),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .visible_alias("cikti")
                .value_name("PDF")
                .value_parser(value_parser!(PathBuf))
                .help(t!(
                    "Çıktı PDF'in adı (yalnız tek EPUB ile)",
                    "Name of the output PDF (single EPUB only)"
                )),
        )
        .arg(
            Arg::new("size")
                .long("size")
                .visible_alias("boyut")
                .value_name(t!("BOYUT", "SIZE"))
                .help(t!(
                    "Sayfa boyutu: {} (varsayılan A4)",
                    "Page size: {} (default A4)",
                    Paper::NAMES
                )),
        )
        .arg(
            Arg::new("font")
                .long("font")
                .value_name(t!("YAZI_TİPİ", "FONT"))
                .help(t!(
                    "Yazı tipi adı ya da .ttf/.otf dosyası (varsayılan Times New Roman)",
                    "Font name or .ttf/.otf file (default Times New Roman)"
                )),
        )
        .arg(
            Arg::new("font-size")
                .long("font-size")
                .visible_alias("punto")
                .value_name("PT")
                .value_parser(value_parser!(f64))
                .help(t!(
                    "Yazı boyutu, punto (varsayılan 12)",
                    "Font size in points (default 12)"
                )),
        )
        .arg(
            Arg::new("line-spacing")
                .long("line-spacing")
                .visible_alias("satir-araligi")
                .value_name(t!("ARALIK", "SPACING"))
                .value_parser(value_parser!(f64))
                .help(t!(
                    "Satır aralığı: 1 tek, 1.5, 2 çift (varsayılan 1.5)",
                    "Line spacing: 1 single, 1.5, 2 double (default 1.5)"
                )),
        )
        .arg(
            Arg::new("margins")
                .long("margins")
                .visible_alias("kenar")
                .value_name("MM")
                .help(t!(
                    "Kenar boşlukları, mm: '25', 'üst,yan,alt' ya da 'üst,iç,dış,alt' (varsayılan 25,40,25,25; iç = cilt tarafı)",
                    "Margins in mm: '25', 'top,sides,bottom' or 'top,inside,outside,bottom' (default 25,40,25,25; inside = binding side)"
                )),
        )
        .arg(
            Arg::new("preview")
                .long("preview")
                .visible_alias("onizleme")
                .value_name(t!("KLASÖR", "DIR"))
                .value_parser(value_parser!(PathBuf))
                .help(t!(
                    "Kapak, bir metin sayfası ve son sayfayı bu klasöre PNG olarak yaz",
                    "Write the cover, a text page and the last page as PNG files here"
                )),
        )
        .arg(config_arg())
        .arg(
            Arg::new("version")
                .short('V')
                .visible_short_alias('v')
                .long("version")
                .action(ArgAction::Version)
                .help(t!("Sürümü göster", "Print version")),
        )
        .subcommand(check)
        .subcommand(info)
        .subcommand(set)
}

fn main_template() -> &'static str {
    match crate::i18n::lang() {
        Lang::Tr => {
            "{about}\n\nKullanım: {usage}\n\nKomutlar:\n{subcommands}\n\nArgümanlar:\n{positionals}\n\nSeçenekler:\n{options}\n"
        }
        Lang::En => {
            "{about}\n\nUsage: {usage}\n\nCommands:\n{subcommands}\n\nArguments:\n{positionals}\n\nOptions:\n{options}\n"
        }
    }
}

fn sub_template() -> &'static str {
    match crate::i18n::lang() {
        Lang::Tr => {
            "{about}\n\nKullanım: {usage}\n\nArgümanlar:\n{positionals}\n\nSeçenekler:\n{options}\n{after-help}"
        }
        Lang::En => {
            "{about}\n\nUsage: {usage}\n\nArguments:\n{positionals}\n\nOptions:\n{options}\n{after-help}"
        }
    }
}

fn run_check(matches: &ArgMatches) -> i32 {
    let path = matches.get_one::<PathBuf>("pdf").expect("required");
    match check::inspect(path) {
        Ok(report) => {
            println!("{}", check::format(&report));
            EXIT_OK
        }
        Err(e) => {
            eprintln!("{}: {e}", t!("Hata", "Error"));
            EXIT_FAILED
        }
    }
}

fn run_info(matches: &ArgMatches) -> i32 {
    let path = matches.get_one::<PathBuf>("epub").expect("required");
    match epub::open(path) {
        Ok(book) => {
            println!("{}", info::format(&info::summarize(&book)));
            EXIT_OK
        }
        Err(e) => {
            eprintln!("{}: {}: {e}", t!("Hata", "Error"), path.display());
            EXIT_FAILED
        }
    }
}

fn run_set(matches: &ArgMatches) -> i32 {
    let explicit = matches.get_one::<PathBuf>("config");
    let Some(path) = config::config_path(explicit.map(PathBuf::as_path)) else {
        eprintln!("{}: {}", t!("Hata", "Error"), ConfigError::NoConfigDir);
        return EXIT_USAGE;
    };

    if matches.get_flag("reset") {
        if path.exists()
            && let Err(e) = std::fs::remove_file(&path)
        {
            eprintln!(
                "{}: {}",
                t!("Hata", "Error"),
                ConfigError::Write(path.clone(), e)
            );
            return EXIT_FAILED;
        }
        println!(
            "{}",
            t!(
                "Ayarlar varsayılanlara döndü.",
                "Settings restored to the defaults."
            )
        );
        return show_settings(&path);
    }

    let words: Vec<&String> = matches
        .get_many::<String>("pairs")
        .map(|v| v.collect())
        .unwrap_or_default();
    if words.is_empty() {
        return show_settings(&path);
    }
    if !words.len().is_multiple_of(2) {
        eprintln!(
            "{}: {}",
            t!("Hata", "Error"),
            t!(
                "her ayar için bir değer verin, örneğin: epub2pdf set punto 11",
                "give a value for each setting, for example: epub2pdf set font-size 11"
            )
        );
        return EXIT_USAGE;
    }
    let mut pairs = Vec::new();
    for pair in words.chunks(2) {
        match Key::parse(pair[0]) {
            Some(key) => pairs.push((key, pair[1].clone())),
            None => {
                eprintln!(
                    "{}: {}",
                    t!("Hata", "Error"),
                    ConfigError::UnknownKey(pair[0].clone())
                );
                return EXIT_USAGE;
            }
        }
    }
    match config::set_values(&path, &pairs) {
        Ok(_) => {
            println!("{}", t!("Kaydedildi.", "Saved."));
            show_settings(&path)
        }
        Err(e @ (ConfigError::Read(..) | ConfigError::Write(..))) => {
            eprintln!("{}: {e}", t!("Hata", "Error"));
            EXIT_FAILED
        }
        Err(e) => {
            eprintln!("{}: {e}", t!("Hata", "Error"));
            EXIT_USAGE
        }
    }
}

/// Prints the settings a conversion would use (without command line options).
fn show_settings(path: &Path) -> i32 {
    let exists = path.exists();
    let file = if exists {
        match config::read_file(path) {
            Ok(file) => file,
            Err(e) => {
                eprintln!("{}: {e}", t!("Hata", "Error"));
                return EXIT_USAGE;
            }
        }
    } else {
        FileConfig::default()
    };
    let settings = match config::combine(&file, &Overrides::default()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {e}", t!("Hata", "Error"));
            return EXIT_USAGE;
        }
    };
    let note = if exists {
        String::new()
    } else {
        t!(" (henüz yok)", " (not created yet)")
    };
    println!(
        "{}: {}{note}",
        t!("Ayar dosyası", "Config file"),
        path.display()
    );
    for key in Key::ALL {
        let origin = if key.is_set_in(&file) {
            String::new()
        } else {
            t!("  (varsayılan)", "  (default)")
        };
        println!("  {:<14} {}{origin}", key.label(), key.display(&settings));
    }
    EXIT_OK
}

fn run_convert(matches: &ArgMatches) -> i32 {
    let overrides = Overrides {
        size: matches.get_one::<String>("size").cloned(),
        font: matches.get_one::<String>("font").cloned(),
        font_size: matches.get_one::<f64>("font-size").copied(),
        line_spacing: matches.get_one::<f64>("line-spacing").copied(),
        margins: matches.get_one::<String>("margins").cloned(),
    };
    let config_file = matches.get_one::<PathBuf>("config");
    let settings = match config::load(config_file.map(PathBuf::as_path), &overrides) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {e}", t!("Hata", "Error"));
            return EXIT_USAGE;
        }
    };

    let inputs: Vec<PathBuf> = matches
        .get_many::<PathBuf>("inputs")
        .expect("required")
        .cloned()
        .collect();
    let output = matches.get_one::<PathBuf>("output");
    let preview = matches.get_one::<PathBuf>("preview");

    let mut summary = Summary::default();
    let books = collect_books(&inputs, &mut summary);
    if output.is_some() && books.len() != 1 {
        eprintln!(
            "{}: {}",
            t!("Hata", "Error"),
            t!(
                "-o/--output yalnız tek bir EPUB ile kullanılabilir",
                "-o/--output can only be used with a single EPUB"
            )
        );
        return EXIT_USAGE;
    }

    for book in &books {
        let target = output
            .cloned()
            .unwrap_or_else(|| book.with_extension("pdf"));
        convert_one(
            book,
            &target,
            &settings,
            preview.map(PathBuf::as_path),
            &mut summary,
        );
    }

    let single = inputs.len() == 1 && books.len() == 1 && inputs[0].is_file();
    if !single {
        println!(
            "{}",
            t!(
                "Özet: {} dönüştürüldü, {} atlandı, {} hatalı",
                "Summary: {} converted, {} skipped, {} failed",
                summary.converted,
                summary.skipped,
                summary.failed
            )
        );
    }
    // A single, explicitly named book that already has a PDF is an error
    // (nothing happened); in batch mode skipping is normal.
    if summary.failed > 0 || (single && summary.skipped > 0) {
        EXIT_FAILED
    } else {
        EXIT_OK
    }
}

#[derive(Default)]
struct Summary {
    converted: usize,
    skipped: usize,
    failed: usize,
}

/// Expands folders into the EPUB files they contain (not recursive).
fn collect_books(inputs: &[PathBuf], summary: &mut Summary) -> Vec<PathBuf> {
    let mut books = Vec::new();
    for input in inputs {
        if input.is_dir() {
            let mut found: Vec<PathBuf> = std::fs::read_dir(input)
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|p| p.is_file() && is_epub(p))
                .collect();
            found.sort();
            if found.is_empty() {
                eprintln!(
                    "{}",
                    t!(
                        "Uyarı: {} içinde EPUB yok",
                        "Warning: no EPUB files in {}",
                        input.display()
                    )
                );
            }
            books.extend(found);
        } else if input.is_file() {
            books.push(input.clone());
        } else {
            eprintln!(
                "{}: {}",
                t!("Hata", "Error"),
                t!(
                    "dosya bulunamadı: {}",
                    "file not found: {}",
                    input.display()
                )
            );
            summary.failed += 1;
        }
    }
    books
}

fn is_epub(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("epub"))
}

fn convert_one(
    source: &Path,
    target: &Path,
    settings: &Settings,
    preview_dir: Option<&Path>,
    summary: &mut Summary,
) {
    if target.exists() {
        println!(
            "{}",
            t!(
                "ATLANDI  {} zaten var, üzerine yazılmaz",
                "SKIPPED  {} already exists and is not overwritten",
                target.display()
            )
        );
        summary.skipped += 1;
        return;
    }
    // The progress line is only useful (and only erasable) on a terminal.
    let terminal = std::io::stdout().is_terminal();
    if terminal {
        print!(
            "{}",
            t!(
                "...      {} dönüştürülüyor",
                "...      converting {}",
                source.display()
            )
        );
        let _ = std::io::stdout().flush();
    }
    let clear = if terminal { "\r\x1b[2K" } else { "" };

    let started = Instant::now();
    match convert_file(source, target, settings, preview_dir) {
        Ok(rendered) => {
            println!(
                "{clear}{}",
                t!(
                    "TAMAM    {} ({} sayfa, {:.1} sn)",
                    "DONE     {} ({} pages, {:.1} s)",
                    target.display(),
                    rendered.page_count,
                    started.elapsed().as_secs_f64()
                )
            );
            for warning in &rendered.warnings {
                eprintln!("         {}: {warning}", t!("uyarı", "warning"));
            }
            for png in &rendered.previews {
                println!("         {}: {}", t!("önizleme", "preview"), png.display());
            }
            summary.converted += 1;
        }
        Err(e) => {
            print!("{clear}");
            let _ = std::io::stdout().flush();
            eprintln!("{}    {}: {e}", t!("HATA", "ERROR"), source.display());
            summary.failed += 1;
        }
    }
}

fn convert_file(
    source: &Path,
    target: &Path,
    settings: &Settings,
    preview_dir: Option<&Path>,
) -> Result<render::Rendered, Box<dyn std::error::Error>> {
    let book = epub::open(source)?;
    let prefix = target
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "page".to_string());
    let preview = preview_dir.map(|dir| Preview {
        dir,
        prefix: &prefix,
    });
    let rendered = render::render(&book, settings, preview)?;

    // Write to a temporary file first so an interrupted run never leaves a
    // half-written PDF under the final name.
    let mut partial = target.as_os_str().to_owned();
    partial.push(".part");
    let partial = PathBuf::from(partial);
    std::fs::write(&partial, &rendered.pdf)?;
    if target.exists() {
        let _ = std::fs::remove_file(&partial);
        return Err(t!(
            "{} bu sırada oluşturuldu, üzerine yazılmadı",
            "{} appeared meanwhile and was not overwritten",
            target.display()
        )
        .into());
    }
    std::fs::rename(&partial, target)?;
    Ok(rendered)
}
