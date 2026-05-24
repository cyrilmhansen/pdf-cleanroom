/// OCR module — architecture placeholder for future optical character recognition.
///
/// OCR is detection-only. It provides optional text extraction from images
/// embedded in PDFs. OCR output is never used as proof that the image
/// content itself was sanitized — pixel-level redaction remains out of scope.
///
/// Currently only `NoopOcrEngine` is provided, which always reports
/// "OCR not available".

/// Result of OCR text extraction on a single image region.
#[derive(Debug, Clone)]
pub struct OcrFinding {
    /// Extracted text from the image region.
    pub text: String,
    /// Confidence score (0.0–1.0). Always 0.0 when OCR is unavailable.
    pub confidence: f64,
}

/// Errors that can occur during OCR processing.
#[derive(Debug)]
pub enum OcrError {
    /// OCR engine is not available / not implemented.
    NotAvailable,
    /// I/O error reading or writing image data.
    Io(std::io::Error),
    /// Processing failure (e.g. corrupt image, unsupported format).
    Processing(String),
}

impl std::fmt::Display for OcrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "OCR engine is not available"),
            Self::Io(e) => write!(f, "OCR I/O error: {e}"),
            Self::Processing(msg) => write!(f, "OCR processing error: {msg}"),
        }
    }
}

impl std::error::Error for OcrError {}

impl From<std::io::Error> for OcrError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Interface for OCR engines.
///
/// An OCR engine extracts text from image byte data (PNG, JPEG, etc.).
/// Implementations may be optional, remote, or disabled.
pub trait OcrEngine: Send + Sync {
    /// Returns `true` if the engine is available and configured.
    fn is_available(&self) -> bool;

    /// Extract text from raw image bytes.
    ///
    /// Returns `Err(OcrError::NotAvailable)` when the engine is not available.
    fn extract_text(&self, image_bytes: &[u8]) -> Result<Vec<OcrFinding>, OcrError>;

    /// A human-readable name for this engine (e.g. "Tesseract", "Noop").
    fn name(&self) -> &'static str;
}

/// A no-op OCR engine that always reports "not available".
///
/// This is the default engine used when no real OCR backend is configured.
/// It never claims to have detected anything.
#[derive(Debug, Clone, Copy)]
pub struct NoopOcrEngine;

impl NoopOcrEngine {
    pub const fn new() -> Self {
        Self
    }
}

impl OcrEngine for NoopOcrEngine {
    fn is_available(&self) -> bool {
        false
    }

    fn extract_text(&self, _image_bytes: &[u8]) -> Result<Vec<OcrFinding>, OcrError> {
        Err(OcrError::NotAvailable)
    }

    fn name(&self) -> &'static str {
        "NoopOcrEngine (not available)"
    }
}

/// Convenience: create the default (no-op) OCR engine.
pub fn default_engine() -> NoopOcrEngine {
    NoopOcrEngine
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_is_not_available() {
        let engine = NoopOcrEngine;
        assert!(!engine.is_available());
        assert_eq!(engine.name(), "NoopOcrEngine (not available)");
    }

    #[test]
    fn test_noop_extract_returns_not_available() {
        let engine = NoopOcrEngine;
        let result = engine.extract_text(b"fake-image-bytes");
        match result {
            Err(OcrError::NotAvailable) => {} // expected
            other => panic!("expected NotAvailable, got {other:?}"),
        }
    }

    #[test]
    fn test_ocr_placeholder_is_explicit() {
        // Requesting OCR when no backend is available must fail clearly.
        let engine = default_engine();
        assert!(!engine.is_available());
        let err = engine.extract_text(b"some-image-data").unwrap_err();
        assert!(err.to_string().contains("not available"));
    }
}
