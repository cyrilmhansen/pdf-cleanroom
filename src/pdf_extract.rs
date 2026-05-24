/// PDF extraction module — text extraction via lopdf.
///
/// Provides both the concrete extraction functions and a `PdfTextExtractor`
/// trait so a future pdfium-based extractor with text positions can be
/// added later.

use lopdf::Document;

/// Extracted content from a single page.
#[derive(Debug, Clone)]
pub struct PageContent {
    pub page_num: usize,
    pub text: String,
}

/// Full extracted document.
#[derive(Debug, Clone)]
pub struct PdfContent {
    pub pages: Vec<PageContent>,
    pub num_pages: usize,
}

/// Trait for PDF text extraction backends.
///
/// The default implementation (`lopdf`-based) returns text without
/// position information. A future implementation using `pdfium-render`
/// or similar could provide text bounding boxes and better encoding
/// support.
pub trait PdfTextExtractor {
    /// Extract text content from a PDF file.
    fn extract(&self, path: &str) -> Result<PdfContent, PdfExtractError>;

    /// Extract text from PDF bytes in memory.
    fn extract_bytes(&self, data: &[u8]) -> Result<PdfContent, PdfExtractError>;
}

/// lopdf-based text extractor.
///
/// This is the default production extractor. It uses lopdf's built-in
/// `extract_text` which handles basic PDF text operators (Tj, TJ, etc.)
/// but does NOT provide text positions and cannot extract from:
/// - Annotations, form fields, embedded files
/// - Non-standard encodings or compressed streams lopdf cannot decode
/// - Images (no OCR)
#[derive(Debug, Clone, Copy)]
pub struct LopdfExtractor;

impl PdfTextExtractor for LopdfExtractor {
    fn extract(&self, path: &str) -> Result<PdfContent, PdfExtractError> {
        extract(path)
    }

    fn extract_bytes(&self, data: &[u8]) -> Result<PdfContent, PdfExtractError> {
        extract_bytes(data)
    }
}

/// Errors during PDF extraction.
#[derive(Debug)]
pub enum PdfExtractError {
    Open(String),
    Extract(String),
    Io(std::io::Error),
}

impl std::fmt::Display for PdfExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open(msg) => write!(f, "cannot open PDF: {msg}"),
            Self::Extract(msg) => write!(f, "text extraction failed: {msg}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for PdfExtractError {}

impl From<std::io::Error> for PdfExtractError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Extract text content from a PDF file using lopdf.
///
/// This uses lopdf's built-in `extract_text` which handles basic PDF
/// text operators (Tj, TJ, etc.). It does NOT extract text from:
/// - Annotations, form fields, embedded files
/// - Non-standard encodings or compressed streams lopdf cannot decode
/// - Images (no OCR)
pub fn extract(path: &str) -> Result<PdfContent, PdfExtractError> {
    let doc =
        Document::load(path).map_err(|e| PdfExtractError::Open(e.to_string()))?;

    let page_map = doc.get_pages();
    let num_pages = page_map.len();
    let mut pages = Vec::with_capacity(num_pages);

    for (page_num, _object_id) in &page_map {
        let text = doc
            .extract_text(&[*page_num])
            .map_err(|e| PdfExtractError::Extract(e.to_string()))?;

        pages.push(PageContent {
            page_num: *page_num as usize,
            text,
        });
    }

    // Sort by page number for deterministic ordering
    pages.sort_by_key(|p| p.page_num);

    Ok(PdfContent { pages, num_pages })
}

/// Extract text from PDF bytes in memory (no filesystem path needed).
pub fn extract_bytes(data: &[u8]) -> Result<PdfContent, PdfExtractError> {
    let doc =
        Document::load_mem(data).map_err(|e| PdfExtractError::Open(e.to_string()))?;

    let page_map = doc.get_pages();
    let num_pages = page_map.len();
    let mut pages = Vec::with_capacity(num_pages);

    for (page_num, _object_id) in &page_map {
        let text = doc
            .extract_text(&[*page_num])
            .map_err(|e| PdfExtractError::Extract(e.to_string()))?;

        pages.push(PageContent {
            page_num: *page_num as usize,
            text,
        });
    }

    pages.sort_by_key(|p| p.page_num);

    Ok(PdfContent { pages, num_pages })
}
