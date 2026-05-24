/// Rebuild module — creates a new clean PDF from extracted text using printpdf.

use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

use crate::detect::Detection;
use crate::mask::{self, MaskMode};

/// A page with its cleaned text (secrets replaced), line by line.
#[derive(Debug, Clone)]
pub struct CleanPage {
    pub page_num: usize,
    pub lines: Vec<String>,
}

/// Build a new PDF from cleaned text.
///
/// This creates a structurally independent PDF document using printpdf.
/// No source PDF objects, metadata, annotations, scripts, or embedded
/// files are copied. Only the visible text is recreated in a new layout.
pub fn rebuild(
    output_path: &str,
    pages: &[CleanPage],
    metadata: &RebuildMetadata,
) -> Result<(), RebuildError> {
    let (doc, first_page, first_layer) = PdfDocument::new(
        &metadata.title,
        Mm(210.0),
        Mm(297.0),
        "Content",
    );

    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| RebuildError::Font(e.to_string()))?;

    // Render first page (already created by PdfDocument::new)
    let mut page_iter = pages.iter().peekable();

    if let Some(clean_page) = page_iter.next() {
        let layer = doc.get_page(first_page).get_layer(first_layer);
        render_page(&layer, clean_page, &font)?;
    }

    // Render subsequent pages
    for clean_page in page_iter {
        let (page_idx, layer_idx) = doc.add_page(Mm(210.0), Mm(297.0), "Content");
        let layer = doc.get_page(page_idx).get_layer(layer_idx);
        render_page(&layer, clean_page, &font)?;
    }

    doc.save(&mut BufWriter::new(
        File::create(output_path).map_err(RebuildError::Io)?,
    ))
    .map_err(|e| RebuildError::Write(e.to_string()))?;

    Ok(())
}

/// Render text from a clean page onto a PDF layer.
fn render_page(
    layer: &PdfLayerReference,
    clean_page: &CleanPage,
    font: &IndirectFontRef,
) -> Result<(), RebuildError> {
    let mut y_pos = 270.0_f32; // Start near top of A4 (origin is bottom-left)

    for line in &clean_page.lines {
        if y_pos < 20.0 {
            break; // Bottom margin reached
        }

        layer.use_text(line, 11.0, Mm(20.0), Mm(y_pos), font);

        // Simple line spacing — best-effort layout
        y_pos -= 6.0;
    }

    Ok(())
}

/// Metadata for the rebuilt PDF document.
pub struct RebuildMetadata {
    pub title: String,
}

/// Errors during PDF rebuild.
#[derive(Debug)]
pub enum RebuildError {
    Io(std::io::Error),
    Font(String),
    Write(String),
}

impl std::fmt::Display for RebuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Font(msg) => write!(f, "font error: {msg}"),
            Self::Write(msg) => write!(f, "write error: {msg}"),
        }
    }
}

impl std::error::Error for RebuildError {}

impl From<std::io::Error> for RebuildError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Apply masks to all detections in a text, returning the cleaned text
/// and the list of replacements performed.
pub fn apply_masks_to_text(
    text: &str,
    detections: &[Detection],
    mode: MaskMode,
) -> (String, Vec<(usize, String)>) {
    if detections.is_empty() {
        return (text.to_string(), Vec::new());
    }

    // Sort detections by start position in reverse order (process from end)
    // so that earlier positions stay valid as we replace.
    let mut sorted: Vec<&Detection> = detections.iter().collect();
    sorted.sort_by(|a, b| b.start.cmp(&a.start));

    let mut result = text.to_string();
    let mut replacements = Vec::new();

    for det in &sorted {
        let mask = mask::apply_mask(&det.value, &det.kind, mode);
        // In replace_range, start and end are byte offsets
        if det.start <= det.end && det.end <= result.len() {
            result.replace_range(det.start..det.end, &mask);
            replacements.push((det.start, mask));
        }
    }

    (result, replacements)
}
