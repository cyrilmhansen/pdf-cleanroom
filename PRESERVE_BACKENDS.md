# Preserve Backend Investigation

Goal: Find a genuine destructive redaction backend for `preserve` mode —
no overlay rectangles, no fake removal.

## Tool Role Separation

Different PDF tools serve fundamentally different roles. Mixing them up leads
to broken security guarantees. Here is the separation this project maintains:

| Role | Tools | Used for |
|---|---|---|
| **Text extraction** | `lopdf`, (future: `pdfium-render`) | Reading text content from PDFs |
| **Rendering** (page → raster) | `pdftoppm` (Poppler), `mutool` (MuPDF), Chromium headless | Visual inspection of test outputs |
| **Structural inspection** | `qpdf` CLI | Verifying absence of annotations, attachments, AcroForm, metadata, encryption in output PDFs |
| **Destructive redaction** | `redactor` crate / MuPDF (not integrated yet) | Physically removing secrets from PDF content streams |
| **OCR** | Tesseract (not integrated yet) | Extracting text from raster images |
| **PDF generation** | `lopdf`, `printpdf` | Creating test fixtures, rebuilding clean PDFs |

**Key rule**: A rendering tool cannot verify structural absence. A structural
inspector cannot perform redaction. Each role needs the right tool.

---

## Candidate 1 — `redactor` crate (MuPDF-based) — Destructive redaction

| Property | Value |
|---|---|
| **Crate** | `redactor` 0.3.0 |
| **Method** | Searches text, creates MuPDF redaction annotations, applies `pdf_redact_page` |
| **Result** | Text is physically removed — not extractable from output, not present in raw bytes |
| **Role** | **Destructive redaction** |
| **Native deps** | Yes — bundles MuPDF via `mupdf-sys` (compiled from C source during build) |
| **License** | `redactor` is MIT; `mupdf-sys` is AGPL-3.0 |
| **API** | File-path based (`redact(input_path, output_path, &[RedactionTarget])`) |
| **Build time** | Significant — MuPDF C source compilation adds minutes |
| **Status** | Not integrated — would require optional feature gate |

**Verdict**: Technically suitable but heavy. AGPL licensing of `mupdf-sys` may be
a concern for downstream use.

---

## Candidate 2 — `pdfium-render` (Pdfium-based) — Rendering + future extraction

| Property | Value |
|---|---|
| **Crate** | `pdfium-render` 0.9.1 |
| **Role** | **Rendering** (page → bitmap), **text extraction with positions** (future) |
| **Redaction support** | None — no editing/writing API |
| **License** | MIT / Apache-2.0 |

**Verdict**: Not suitable for redaction. Could serve as a future `PdfTextExtractor`
backend that provides text positions (unlike lopdf), but that is separate work.

---

## Candidate 3 — `qpdf` CLI — Structural inspection (optional tests)

`qpdf` is not a Rust crate — it is a standalone CLI tool for structural PDF
inspection and rewriting. It can be used in **optional CI/developer tests**
(if installed) to verify output PDFs, but must never be a build dependency.

### Capabilities tested

| Feature | `qpdf --json=2` exposes |
|---|---|
| **Annotations** | `/Annots` per page, full annotation objects (`/Contents`, `/Rect`, `/Subtype`) |
| **Embedded files** | `attachments` dict with filename keys |
| **AcroForm / form fields** | `acroform.fields` array with field type, default value, flags |
| **Metadata (Info dict)** | Trailer `/Info` dictionary (Title, Author, Subject, etc.) |
| **Encryption** | `encrypt` section (method, key length, capabilities) |
| **Page structure** | Page tree, MediaBox, CropBox, contents, resources, fonts, images |
| **XMP metadata** | Metadata stream reference in Catalog |
| **Object graph** | Full object tree with raw values |

### Usage in tests (optional, if `qpdf` is installed)

```rust
fn qpdf_json(path: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("qpdf")
        .arg("--json=2")
        .arg(path)
        .output()?;
    Ok(serde_json::from_slice(&output.stdout)?)
}
```

This can verify:
- "rebuilt PDF has no annotations" → `annotations` list is empty
- "rebuilt PDF has no attachments" → `attachments` dict is empty
- "rebuilt PDF has no AcroForm" → `acroform.hasacroform` is false
- "flattened output has no source metadata" → Info dict contains only pdf-cleanroom metadata

**qpdf must not be used for redaction, text extraction, or rendering.**

---

## Candidate 4 — `lopdf` (current)

| Property | Value |
|---|---|
| **Status** | Used for extraction and test fixtures |
| **Role** | **Text extraction** (no positions), **low-level PDF construction** |
| **Redaction** | Must not be used for "redaction by overlay" — that leaves secrets extractable |

---

## Candidate 5 — Tesseract / OCR

| Property | Value |
|---|---|
| **Role** | OCR — extracting text from raster images |
| **Integration** | Not begun |

Not relevant to preserve mode. Documented for completeness.

---

## Decision

As of this writing, `preserve` remains **unimplemented**. The CLI error message
now explains which backend would be needed:

> preserve mode is not implemented. A real destructive redaction backend
> (such as redactor/MuPDF or similar) must be integrated first.
> pdf-cleanroom will NEVER simulate redaction by overlaying black rectangles.

A future phase may add an optional `--experimental-backend redactor` flag and
feature-gated dependency. Until then, use `rebuild` for clean PDF reconstruction
or the `redactor` standalone CLI directly.

## References

- `redactor` crate: https://crates.io/crates/redactor
- `mupdf-sys` crate: https://crates.io/crates/mupdf-sys
- `pdfium-render` crate: https://crates.io/crates/pdfium-render
- `qpdf` CLI: https://qpdf.sourceforge.io/
- `pikepdf` (Python qpdf wrapper): https://github.com/pikepdf/pikepdf
