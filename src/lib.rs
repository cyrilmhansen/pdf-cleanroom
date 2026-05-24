//! pdf-cleanroom — PDF document sanitization tool.
//!
//! Scans PDF documents for secrets (emails, phone numbers, IBANs)
//! and rebuilds a clean PDF with those secrets masked.

pub mod detect;
pub mod flatten;
pub mod mask;
pub mod ocr;
pub mod pdf_extract;
pub mod rebuild;
pub mod report;
pub mod safety;
pub mod version;
