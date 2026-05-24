/// Support module for integration tests — PDF generation helpers.
///
/// All PDFs are generated at runtime using lopdf and/or printpdf.
/// No binary PDF fixtures are committed.

/// Benchmark metric types — struct definitions only, no rendering.
/// See BENCHMARKS.md for the full benchmark design.
pub mod visual_metrics;

pub mod pdf_fixtures;
