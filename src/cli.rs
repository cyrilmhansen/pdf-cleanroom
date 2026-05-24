/// CLI module — argument parsing with clap.
use clap::{Parser, Subcommand, ValueEnum};

/// Output strategy for sanitized PDFs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Strategy {
    /// Render each page to an image and rebuild an image-only PDF. Preserves
    /// visual appearance and removes selectable text. Does NOT mask visible
    /// secrets unless pixel mask regions are applied.
    FlattenRaster,
    /// Rebuild a fresh PDF from extracted visible text only. Images and
    /// non-text content are not preserved. Intended for text archival / AI
    /// ingestion, not visual fidelity.
    TextOnly,
    /// Future/partial strategy intended to preserve visible content. Currently
    /// incomplete; warns about unsupported image sanitization.
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
                  ⚠ SECURITY WARNING: text-only is not pixel redaction. \
                  flatten-raster creates image-only output. Automatic pixel masks \
                  cover only selectable PDF text-layer secrets; no OCR is used."
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
    #[arg(long, default_value = "text-only", value_enum, global = true)]
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
    ///     pdf-cleanroom --strategy flatten-raster rebuild input.pdf output.pdf --mask-detected
    Rebuild {
        /// Input PDF file path.
        input: String,

        /// Output PDF file path.
        output: String,

        /// Path to write the JSON report.
        #[arg(long)]
        report: Option<String>,

        /// Automatically derive pixel mask regions from selectable PDF text coordinates.
        ///
        /// Only valid with `--strategy flatten-raster`. Requires Poppler
        /// `pdftotext -bbox`; does not OCR images.
        #[arg(long = "mask-detected")]
        mask_detected: bool,
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
