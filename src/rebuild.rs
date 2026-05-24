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

// Layout constants
const FONT_SIZE: f32 = 10.0;
const LINE_HEIGHT: f32 = 14.0; // ~1.4× font size for readable spacing
const LEFT_MARGIN_MM: f32 = 20.0;
const TOP_MARGIN_MM: f32 = 30.0;
const BOTTOM_MARGIN_MM: f32 = 20.0;
const MAX_CHARS_PER_LINE: usize = 85; // conservative for 10pt on A4 with 20mm margins

/// Render text from a clean page onto a PDF layer.
///
/// Line spacing and position are best-effort — lopdf does not provide
/// text coordinates, so we use a simple fixed-layout algorithm.
fn render_page(
    layer: &PdfLayerReference,
    clean_page: &CleanPage,
    font: &IndirectFontRef,
) -> Result<(), RebuildError> {
    let page_height_mm: f32 = 297.0;
    let start_y = page_height_mm - TOP_MARGIN_MM;
    let mut y_pos = start_y;

    for line in &clean_page.lines {
        if y_pos < BOTTOM_MARGIN_MM {
            break; // Bottom margin reached
        }

        if line.is_empty() {
            y_pos -= LINE_HEIGHT;
            continue;
        }

        // Split long lines at word boundaries to avoid text overflow.
        let wrapped_lines = wrap_line(line, MAX_CHARS_PER_LINE);

        for wrapped in &wrapped_lines {
            if y_pos < BOTTOM_MARGIN_MM {
                break;
            }
            layer.use_text(wrapped, FONT_SIZE, Mm(LEFT_MARGIN_MM), Mm(y_pos), font);
            y_pos -= LINE_HEIGHT;
        }
    }

    Ok(())
}

/// Wrap a single line at space boundaries to fit within `max_chars`.
///
/// Operates on **character** indices, not byte indices, to handle
/// multi-byte UTF-8 (mask characters, accents) correctly.
fn wrap_line(line: &str, max_chars: usize) -> Vec<String> {
    if line.chars().count() <= max_chars {
        return vec![line.to_string()];
    }

    let mut result = Vec::new();
    let mut remaining = line;

    while !remaining.is_empty() {
        let char_count = remaining.chars().count();
        if char_count <= max_chars {
            result.push(remaining.to_string());
            break;
        }

        // Find character at position max_chars, then look back for a space.
        let byte_end: usize = remaining
            .chars()
            .take(max_chars)
            .map(|c| c.len_utf8())
            .sum();

        // Look back from byte_end for the last space.
        let slice = &remaining[..byte_end];
        if let Some(last_space) = slice.rfind(' ') {
            let (head, tail) = remaining.split_at(last_space);
            result.push(head.to_string());
            remaining = tail.trim_start();
        } else {
            // No space found — hard-break at character boundary.
            let char_end: usize = remaining
                .chars()
                .take(max_chars)
                .map(|c| c.len_utf8())
                .sum();
            let (head, tail) = remaining.split_at(char_end);
            result.push(head.to_string());
            remaining = tail;
        }
    }

    result
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
