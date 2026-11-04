//! Writes the synthetic sample EPUBs used by the tests into a folder.
//!
//! ```text
//! cargo run --example sample_epub -- OUTPUT_DIR
//! ```

#[path = "../tests/common/mod.rs"]
mod common;

fn main() {
    let dir = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "samples".into());
    match common::write_samples(&dir) {
        Ok(paths) => {
            for path in paths {
                println!("{}", path.display());
            }
        }
        Err(e) => {
            eprintln!("cannot write samples to {}: {e}", dir.display());
            std::process::exit(1);
        }
    }
}
