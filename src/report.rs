/// Report module — JSON report for detected secrets.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write;

/// A single detected secret occurrence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SecretOccurrence {
    /// 1-indexed page number.
    pub page: usize,
    /// Kind of secret (e.g. "email", "phone_fr", "iban").
    pub kind: String,
    /// Display-safe masked version.
    pub masked_display: String,
    /// SHA-256 hex digest of the exact value.
    pub sha256: String,
    /// Character offset start in extracted text (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    /// Character offset end in extracted text (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<usize>,
    /// Approximate coordinates if available from the extraction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coords: Option<Coords>,
    /// Exact value — only present when --unsafe-show-secrets is set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exact_value: Option<String>,

    /// Source of the detection (e.g. "pdf_text", "metadata", "xmp_metadata",
    /// "annotation", "form", "attachment", "ocr").
    /// When absent, the source is `"pdf_text"` (extracted visible text).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Approximate bounding box.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coords {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Full scan report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Report {
    /// Number of pages scanned.
    pub pages_scanned: usize,
    /// Total secrets detected.
    pub total_secrets: usize,
    /// Per-kind counts.
    pub by_kind: std::collections::HashMap<String, usize>,
    /// Individual occurrences.
    pub occurrences: Vec<SecretOccurrence>,
    /// Whether exact values were included.
    pub unsafe_show_secrets: bool,
    /// CLI tool version.
    pub tool: String,
    /// SHA-256 of the input document (raw bytes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_sha256: Option<String>,
}

impl Report {
    pub fn new(pages_scanned: usize, unsafe_show_secrets: bool) -> Self {
        Self {
            pages_scanned,
            total_secrets: 0,
            by_kind: std::collections::HashMap::new(),
            occurrences: Vec::new(),
            unsafe_show_secrets,
            tool: format!("pdf-cleanroom/{}", env!("CARGO_PKG_VERSION")),
            input_sha256: None,
        }
    }

    pub fn add_occurrence(
        &mut self,
        page: usize,
        kind: impl Into<String>,
        exact_value: Option<String>,
        start: Option<usize>,
        end: Option<usize>,
        coords: Option<Coords>,
        masked: String,
    ) {
        let kind: String = kind.into();
        let sha256 = hex_sha256(exact_value.as_deref().unwrap_or(&masked));

        let masked_display = if self.unsafe_show_secrets {
            exact_value
                .as_deref()
                .unwrap_or(&masked)
                .to_string()
        } else {
            masked
        };

        let occurrence = SecretOccurrence {
            page,
            kind: kind.clone(),
            masked_display,
            sha256,
            start,
            end,
            coords,
            exact_value: if self.unsafe_show_secrets {
                exact_value
            } else {
                None
            },
            source: None,
        };
        self.total_secrets += 1;
        *self.by_kind.entry(kind).or_insert(0) += 1;
        self.occurrences.push(occurrence);
    }

    /// Add a detected secret occurrence with an explicit source label.
    ///
    /// `source` describes where the detection originated, e.g.
    /// `"pdf_text"`, `"metadata"`, `"xmp_metadata"`, `"annotation"`,
    /// `"form"`, `"attachment"`, or `"ocr"`.
    pub fn add_occurrence_with_source(
        &mut self,
        page: usize,
        kind: impl Into<String>,
        exact_value: Option<String>,
        start: Option<usize>,
        end: Option<usize>,
        coords: Option<Coords>,
        masked: String,
        source: &str,
    ) {
        self.add_occurrence(page, kind, exact_value, start, end, coords, masked);
        if let Some(occ) = self.occurrences.last_mut() {
            occ.source = Some(source.to_string());
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

pub fn hex_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in result {
        write!(hex, "{byte:02x}").unwrap();
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_add_occurrence() {
        let mut report = Report::new(2, false);
        report.add_occurrence(
            1,
            "email",
            Some("user@example.com".into()),
            Some(10),
            Some(26),
            None,
            "[EMAIL]".into(),
        );
        assert_eq!(report.total_secrets, 1);
        assert_eq!(report.by_kind.get("email"), Some(&1));
        // Without unsafe-show-secrets, masked_display is the mask, not the value
        assert_eq!(report.occurrences[0].masked_display, "[EMAIL]");
        assert!(report.occurrences[0].exact_value.is_none());
    }

    #[test]
    fn test_report_unsafe_shows_exact_value() {
        let mut report = Report::new(1, true);
        report.add_occurrence(
            1,
            "email",
            Some("user@example.com".into()),
            None,
            None,
            None,
            "[EMAIL]".into(),
        );
        // With unsafe-show-secrets, masked_display shows the exact value
        assert_eq!(
            report.occurrences[0].masked_display,
            "user@example.com"
        );
        assert_eq!(
            report.occurrences[0].exact_value,
            Some("user@example.com".into())
        );
    }

    #[test]
    fn test_hex_sha256() {
        let hash = hex_sha256("hello");
        assert_eq!(hash.len(), 64);
        // Known SHA-256 of "hello"
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_report_to_json() {
        let mut report = Report::new(1, false);
        report.input_sha256 = Some("abc".into());
        report.add_occurrence(
            1,
            "phone_fr",
            Some("06 11 22 33 44".into()),
            None,
            None,
            None,
            "[PHONE]".into(),
        );
        let json = report.to_json().unwrap();
        assert!(json.contains("phone_fr"));
        assert!(json.contains("06 11 22 33 44") == false);
        assert!(json.contains("[PHONE]"));
    }
}
