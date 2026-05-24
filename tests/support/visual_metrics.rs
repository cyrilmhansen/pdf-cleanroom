//! Visual fidelity benchmark support — struct definitions only.
//!
//! These types describe the input and output of a visual benchmark run.
//! The actual rendering, comparison, and metric computations will be added
//! in a later phase. No renderer tools are required to compile this module.
//!
//! # Usage (future)
//!
//! ```ignore
//! use pdf_cleanroom_bench::*;
//!
//! let result = VisualBenchmarkResult::new("text-only");
//! let metrics = PageVisualMetrics::new(1, 1240, 1754);
//! ```


// ---------------------------------------------------------------------------
// Rendered page
// ---------------------------------------------------------------------------

/// A single page rendered to a raster image.
///
/// Pixel data is stored as raw RGBA bytes (row-major, top-to-bottom).
/// This may be `None` if rendering has not yet been performed.
#[derive(Debug, Clone)]
pub struct RenderedPage {
    /// 1-indexed page number.
    pub page_num: usize,

    /// Pixel width of the rendered image.
    pub width: u32,

    /// Pixel height of the rendered image.
    pub height: u32,

    /// Rendering DPI used.
    pub dpi: u32,

    /// Optional raw RGBA pixel data (width × height × 4 bytes).
    /// `None` means the page was not rendered (e.g. renderer unavailable).
    pub pixels: Option<Vec<u8>>,
}

// ---------------------------------------------------------------------------
// Mask region
// ---------------------------------------------------------------------------

/// A rectangular region that has been intentionally masked/redacted.
///
/// When comparing input and output, pixels inside mask regions are excluded
/// from fidelity metrics (they are expected to differ) and compared separately
/// to confirm the secret has been removed.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskRegion {
    /// Page number (1-indexed).
    pub page: usize,

    /// Bounding box in pixel coordinates.
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,

    /// Optional label describing what was masked (e.g. "email", "iban").
    pub label: Option<String>,
}

// ---------------------------------------------------------------------------
// Per-page metrics
// ---------------------------------------------------------------------------

/// Pixel-level fidelity metrics for a single page.
#[derive(Debug, Clone, PartialEq)]
pub struct PixelMetrics {
    /// Mean absolute pixel error (0–255). Lower = better.
    pub mae: f64,

    /// Peak signal-to-noise ratio in dB. Higher = better.
    pub psnr: f64,

    /// Structural similarity index (0–1). Higher = better.
    pub ssim: f64,

    /// Simplified SSIM (0–1). Higher = better.
    pub s_ssim: f64,

    /// Perceptual hash similarity (0–1). Higher = better.
    pub phash_similarity: f64,
}

/// Sanitisation checks for one page of the output PDF.
#[derive(Debug, Clone, PartialEq)]
pub struct SanitizationChecks {
    /// Number of secrets detected in the input page text.
    pub secrets_found_input: usize,

    /// Number of secrets detected in the output page text.
    pub secrets_found_output: usize,

    /// Whether original secret strings appear in raw output bytes.
    pub raw_byte_leak: bool,

    /// Whether the output has annotations (should be false).
    pub annotations_present: bool,

    /// Whether the output has embedded file attachments (should be false).
    pub attachments_present: bool,

    /// Whether the output has AcroForm fields (should be false).
    pub acroform_present: bool,

    /// Whether the output is encrypted (should be false).
    pub encrypted: bool,
}

/// Metrics computed for one page of a benchmark run.
#[derive(Debug, Clone)]
pub struct PageVisualMetrics {
    /// 1-indexed page number.
    pub page: usize,

    /// Rendered pixel dimensions.
    pub width_px: u32,
    pub height_px: u32,

    /// File size ratio (output_size / input_size).
    pub size_ratio: f64,

    /// Timing in milliseconds.
    pub render_ms: u64,
    pub process_ms: u64,
    pub compare_ms: u64,

    /// Fidelity metrics for unmasked regions (higher = better).
    pub unmasked: Option<PixelMetrics>,

    /// Fidelity metrics for masked regions (here, lower similarity = better).
    pub masked: Option<PixelMetrics>,

    /// Number of mask regions on this page.
    pub mask_region_count: usize,

    /// Sanitisation check results.
    pub sanitization: Option<SanitizationChecks>,

    /// Optional OCR anchor recall for this page.
    /// `None` when OCR is not available.
    pub ocr_anchor_recall: Option<f64>,
}

// ---------------------------------------------------------------------------
// Overall benchmark result
// ---------------------------------------------------------------------------

/// Configuration for a single benchmark run.
#[derive(Debug, Clone)]
pub struct BenchConfig {
    /// Output strategy used (e.g. "text-only", "flatten-visible", "preserve").
    pub strategy: String,

    /// Mask mode (e.g. "black-block", "label", "hash").
    pub mask_mode: String,

    /// Rendering DPI.
    pub dpi: u32,

    /// Name of the device-under-test (e.g. "pdf-cleanroom 0.1.0").
    pub dut: String,
}

/// Summary statistics for a complete benchmark run.
#[derive(Debug, Clone)]
pub struct BenchSummary {
    pub total_pages: usize,
    pub mean_unmasked_psnr: f64,
    pub mean_unmasked_ssim: f64,
    pub mean_masked_mae: f64,
    pub size_ratio: f64,
    pub total_render_ms: u64,
    pub total_process_ms: u64,
    pub total_compare_ms: u64,
    pub all_sanitization_checks_passed: bool,
}

/// Top-level result of a single visual benchmark run.
#[derive(Debug, Clone)]
pub struct VisualBenchmarkResult {
    /// Schema version for the metrics output.
    pub benchmark_version: u32,

    /// ISO 8601 timestamp of the run.
    pub timestamp: String,

    /// Run configuration.
    pub config: BenchConfig,

    /// Per-page metrics.
    pub pages: Vec<PageVisualMetrics>,

    /// Aggregate summary.
    pub summary: BenchSummary,

    /// Paths to generated files (relative to bench output dir).
    pub input_pdf: String,
    pub output_pdf: String,
    pub render_dir: String,
}

impl VisualBenchmarkResult {
    /// Create a new benchmark result with the given strategy name.
    pub fn new(strategy: &str) -> Self {
        Self {
            benchmark_version: 1,
            timestamp: String::new(),
            config: BenchConfig {
                strategy: strategy.to_string(),
                mask_mode: "label".to_string(),
                dpi: 150,
                dut: "pdf-cleanroom 0.1.0".to_string(),
            },
            pages: Vec::new(),
            summary: BenchSummary {
                total_pages: 0,
                mean_unmasked_psnr: 0.0,
                mean_unmasked_ssim: 0.0,
                mean_masked_mae: 0.0,
                size_ratio: 0.0,
                total_render_ms: 0,
                total_process_ms: 0,
                total_compare_ms: 0,
                all_sanitization_checks_passed: false,
            },
            input_pdf: String::new(),
            output_pdf: String::new(),
            render_dir: String::new(),
        }
    }

    /// Add a page to the result.
    pub fn add_page(&mut self, page: PageVisualMetrics) {
        self.pages.push(page);
    }

    /// Finalise the summary from collected page metrics.
    pub fn finalise(&mut self) {
        let n = self.pages.len() as f64;
        if n == 0.0 {
            return;
        }
        self.summary.total_pages = self.pages.len();
        self.summary.mean_unmasked_psnr = self
            .pages
            .iter()
            .filter_map(|p| p.unmasked.as_ref().map(|m| m.psnr))
            .sum::<f64>()
            / n;
        self.summary.mean_unmasked_ssim = self
            .pages
            .iter()
            .filter_map(|p| p.unmasked.as_ref().map(|m| m.ssim))
            .sum::<f64>()
            / n;
        self.summary.mean_masked_mae = self
            .pages
            .iter()
            .filter_map(|p| p.masked.as_ref().map(|m| m.mae))
            .sum::<f64>()
            / n;
        self.summary.size_ratio = self.pages.first().map_or(0.0, |p| p.size_ratio);
        self.summary.total_render_ms = self.pages.iter().map(|p| p.render_ms).sum();
        self.summary.total_process_ms = self.pages.iter().map(|p| p.process_ms).sum();
        self.summary.total_compare_ms = self.pages.iter().map(|p| p.compare_ms).sum();
        self.summary.all_sanitization_checks_passed = self
            .pages
            .iter()
            .all(|p| p.sanitization.as_ref().map_or(false, |s| {
                s.secrets_found_output == 0
                    && !s.raw_byte_leak
                    && !s.annotations_present
                    && !s.attachments_present
                    && !s.acroform_present
                    && !s.encrypted
            }));
    }
}

// ---------------------------------------------------------------------------
// Helper: default MaskRegion from a bounding box
// ---------------------------------------------------------------------------

impl MaskRegion {
    /// Create a new mask region.
    pub fn new(page: usize, x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            page,
            x,
            y,
            width,
            height,
            label: None,
        }
    }

    /// Attach a label to this region.
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }
}
