/// CLI module — argument parsing with clap.

use clap::{Parser, Subcommand, ValueEnum};



/// Output strategy for sanitized PDFs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Strategy {
    /// Rebuild a fresh PDF from extracted visible text only. Images are not preserved.
    /// Render each page to an image and rebuild an image-only PDF.
    /// Preserves visual appearance; removes selectable text. Does NOT mask
    /// visible secrets in the rendered image.
    FlattenRaster,
    TextOnly,
    /// Preserve visible content including images (partial — images are not yet sanitized).
    /// Warns about unsupported image sanitization.
    FlattenVisible,
}

impl std::fmt::Display for Strategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FlattenRaster => write!(f, "flatten-raster"),
            Self::TextOnly => write!(f, "text-only"),
            Self::FlattenVisible => write!(f, "flatten-visible"),
        }

    }
}


#[derive(Parser)]
#[command(
    name = "pdf-cleanroom",
    about = "PDF document sanitization — detect and remove secrets from PDF files",
    long_about = "pdf-cleanroom scans PDF documents for secrets (emails, phone numbers, IBANs) \
                  and rebuilds a clean PDF with those secrets masked or removed.\n\n\
                  ⚠ SECURITY WARNING: pdf-cleanroom rebuilds the PDF from extracted text. \
                  It does NOT perform pixel-level redaction. Embedded images, annotations, \
                  and non-extracted text may still contain secrets. This tool reduces \
                  exposure but does not guarantee complete sanitization."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Perform analysis and generate report, but do not produce a modified PDF.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Allow exact secret values to appear in console and report output.
    /// Without this flag, secrets are masked or hashed in all output.
    #[arg(long = "unsafe-show-secrets", global = true)]
    pub unsafe_show_secrets: bool,

    /// Masking style for rebuild mode.
    #[arg(
        long,
        default_value = "black-block",
        value_parser = ["black-block", "label", "same-width", "thin-air", "hash"],
        global = true
    )]
    pub mask: String,

    /// Output strategy for sanitized PDFs.
    ///
    /// - `flatten-raster` (experimental): render each page to an image and
    ///   rebuild an image-only PDF.  Visually faithful; no selectable text.
    ///   Does NOT mask visible secrets in the rendered image.
    /// - `text-only` (default): rebuild a fresh PDF from extracted visible text only.
    ///   Images and non-text content are not preserved.
    /// - `flatten-visible`: intended to preserve visible content including images.
    ///   Currently partial — images are not yet sanitized. A warning is emitted
    ///   when this strategy is selected.
    #[arg(
        long,
        default_value = "text-only",
        value_enum,
        global = true
    )]
    pub strategy: Strategy,
}

#[derive(Subcommand)]
pub enum Command {
    /// Scan a PDF for secrets and produce a JSON report.
    Scan {
        /// Input PDF file path.
        input: String,

        /// Path to write the JSON report.
        #[arg(long)]
        report: Option<String>,
    },

    /// Rebuild a PDF with secrets masked.
    ///
    /// Extracts visible text from the input PDF, detects secrets (emails,
    /// French phone numbers, IBANs), replaces them with masked text, and
    /// writes a clean PDF to OUTPUT.
    ///
    /// The output is a fresh printpdf document — source PDF structure
    /// (metadata, annotations, forms, attachments) is never copied.
    ///
    /// # Examples
    ///
    ///     pdf-cleanroom rebuild input.pdf output.pdf
    ///     pdf-cleanroom rebuild input.pdf output.pdf --mask label
    ///     pdf-cleanroom rebuild input.pdf output.pdf --report report.json
    Rebuild {
        /// Input PDF file path.
        input: String,

        /// Output PDF file path.
        output: String,

        /// Path to write the JSON report.
        #[arg(long)]
        report: Option<String>,
    },

    /// Preserve fidelity mode (NOT IMPLEMENTED).
    ///
    /// This command will refuse to run until a real destructive redaction
    /// is implemented. pdf-cleanroom will NEVER redact by overlaying
    /// black rectangles.
    Preserve {
        /// Input PDF file path.
        input: String,

        /// Output PDF file path.
        output: String,
    },
}
