fn main() {
    std::process::exit(epub2pdf::cli::run(std::env::args_os()));
}
