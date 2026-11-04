//! epub2pdf: convert EPUB books into readable PDFs.
//!
//! The pipeline is:
//! 1. [`epub`] reads the archive (container, package document, spine, metadata).
//! 2. [`convert`] turns every XHTML chapter into Typst markup.
//! 3. [`render`] typesets the markup with an embedded font and writes the PDF.
//!
//! [`check`] inspects an existing PDF, [`info`] summarises an EPUB and [`cli`]
//! ties everything together behind the `epub2pdf` command.

pub mod i18n;

pub mod check;
pub mod cli;
pub mod config;
pub mod convert;
pub mod epub;
pub mod info;
pub mod render;
