# pdf-cleanroom

**PDF document sanitization — detect and remove secrets from PDF files.**

pdf-cleanroom scans PDF documents for secrets (emails, French phone numbers, IBANs) and rebuilds a clean PDF with those secrets masked or removed.

## ⚠ Security Warning

pdf-cleanroom **reduces exposure** but does **not guarantee complete sanitization**.

- Text extraction is performed by `lopdf` and rebuild by `printpdf`. The source PDF structure (metadata, annotations, form fields, scripts, embedded files) is **never copied** — output is a fresh document built from extracted text only.
- **OCR is out of scope.** Secrets in scanned images or rasterized text are not detected. Since rebuild is text-only, source images are not carried over to output.
- **`--strategy flatten-visible` is experimental.** Image sanitization is not yet implemented. A warning is emitted; current behavior matches `text-only`.
- Text in non-standard encodings or streams `lopdf` cannot decode will not be extracted and may remain undetected.

This is an **experimental research project** developed iteratively with LLM-based tooling. The test suite includes dynamically generated hostile PDF inputs and raw-byte regression checks, but detection and rebuild have known limitations documented in [DESIGN.md](DESIGN.md).

**Do not rely on this tool alone** for classified or legally restricted documents. Use in combination with visual verification and other sanitization tools.

## Usage

```text
$ pdf-cleanroom --help
```

### Scan

Analyze a PDF and produce a JSON report of detected secrets:

```bash
pdf-cleanroom scan input.pdf --report report.json
```

### Rebuild

Extract text, mask secrets, and produce a clean PDF:

```bash
# Basic rebuild (black-block masking)
pdf-cleanroom rebuild input.pdf output.pdf

# Label-based masking
pdf-cleanroom rebuild input.pdf output.pdf --mask label

# With JSON report
pdf-cleanroom rebuild input.pdf output.pdf --report report.json

# Dry-run: analyze only, do not write output
pdf-cleanroom rebuild input.pdf output.pdf --dry-run
```

### Preserve (NOT IMPLEMENTED)

```bash
pdf-cleanroom preserve input.pdf output.pdf
# ERROR: preserve mode is not implemented.
```

pdf-cleanroom will **never** simulate redaction by overlaying black rectangles. The preserve command will remain unimplemented until a real destructive removal can be demonstrated.

## Options

| Flag | Description |
|------|-------------|
| `--dry-run` | Analyze and produce report only; do not write a modified PDF |
| `--unsafe-show-secrets` | Show exact secret values in console and report output |
| `--mask <MODE>` | Masking style for rebuild: `black-block` (default), `label`, `same-width`, `thin-air`, `hash` |

## Masking Modes

| Mode | Example |
|------|---------|
| `black-block` | `████████████████` |
| `label` | `[SUPPRIMÉ: EMAIL]` |
| `same-width` | `████████` (same width as original) |
| `thin-air` | `⟦ supprimé ⟧` |
| `hash` | `[SUPPRIMÉ#8F3A]` |

## Detection

- **Emails** — RFC-compatible pattern
- **French phone numbers** — `0X XX XX XX XX` and `+33 X XX XX XX XX` formats
- **IBAN** — international bank account numbers (all countries)

## Report Format

The JSON report contains per-occurrence entries with:

- `page` — page number (1-indexed)
- `kind` — `"email"`, `"phone_fr"`, or `"iban"`
- `masked_display` — masked version of the secret (or exact value with `--unsafe-show-secrets`)
- `sha256` — SHA-256 hash of the exact value
- `start` / `end` — character offsets in extracted text
- `exact_value` — only present when `--unsafe-show-secrets` is active

## Design Decisions

See [DESIGN.md](DESIGN.md) for the full rationale.

Key points:
- **No black rectangles.** Redacting by overlaying shapes is a known anti-pattern — the hidden text remains in the PDF.
- **No preview mode.** Previews can be confused with real redaction.
- **Rebuild from scratch.** The source PDF structure is never used as a base.
- **Preserve is blocked** until destructive removal is verifiably implemented.

## Build & Test

```bash
cargo build
cargo test
```

## Visual inspection

Generate rendered PNG outputs of all runtime fixture PDFs for manual or CI review:

```sh
PDF_CLEANROOM_VISUAL=1 cargo test -- --ignored visual
```

Output goes to `target/pdf-cleanroom-visual/` (gitignored). Requires one of:
- **pdftoppm** (poppler-utils) — preferred
- **mutool** (MuPDF)
- **Chromium** headless — fallback

If no renderer is available, PDFs are still generated and a clear message is printed.
## Optional OCR smoke tests

End-to-end smoke test of the OCR detection pipeline:

```sh
PDF_CLEANROOM_OCR_TESTS=1 cargo test --test integration_ocr_smoke
```

The test generates a PNG with secret text via Python+Pillow (or skips if
unavailable), runs `tesseract` on the image, and feeds the extracted text
into the regex detector.  It also renders a visible-text PDF to PNG via
`pdftoppm` (or `mutool`) and runs the same pipeline — this validates the
full PDF→render→OCR chain.

It asserts that **at least one** secret is detected — OCR quality varies,
so not all planted secrets are required to match.

If the env var is unset, the test is skipped without noise. No renderer
or OCR tool is needed for normal `cargo test`.

### Prerequisites

| Tool | Role | Install (Arch) |
|---|---|---|
| `tesseract` | OCR engine | `pacman -S tesseract tesseract-data-fra tesseract-data-eng` |
| `pdftoppm` | PDF → PNG renderer | `pacman -S poppler` |
| `mutool` | fallback renderer | `pacman -S mupdf-tools` |
| `python3` + Pillow | text image generation | `pacman -S python python-pillow` |

The French (`fra`) tesseract language pack is recommended — planted IBANs,
phone numbers, and names use French conventions that `fra` handles better
than `eng` alone.

### Limitations

- **Detection-only.** OCR output is never used as proof that image content
  itself was sanitised. Pixel-level redaction remains out of scope.
- **No hidden OCR text layers.** The rebuilt PDF does not include invisible
  text from OCR — that would create a false sense of security.
- **No Rust OCR crates.** The test invokes `tesseract` as an external
  binary via `std::process::Command`.
- **Image-only PDF renders correctly** — the `cm` scaling matrix was added
  to scale the image XObject from the unit square to its pixel dimensions.
  Structural verification (PNG file size) passes.  The 5×7 bitmap font used
  for the embedded image is too crude for reliable Tesseract OCR, so the
  image-only PDF test validates rendering but does NOT assert on detected
  secrets — those are covered by the Pillow text test.

- Generated PDFs and PNGs are written to `target/pdf-cleanroom-ocr/`
  (gitignored).

## Flatten-raster strategy

The `--strategy flatten-raster` option renders each page of the source PDF
to a raster image and rebuilds a fresh image-only PDF from those images.
This preserves visual appearance better than `text-only` (images, layout,
fonts are baked into pixels) but produces output with no PDF text layer.

```sh
# Basic flatten
pdf-cleanroom rebuild input.pdf output.pdf --strategy flatten-raster
```

Some viewers, especially browser PDF viewers, may offer OCR-like visual
text selection on image-only PDFs. This does not mean the source PDF text
layer was preserved. Use structural tools such as `pdftotext`, `qpdf`, and
`pdfimages` to verify the actual PDF contents.

**Important caveats:**
- Selectable text is removed — the output contains only raster images.
- Visible secrets in the rendered images remain visible unless manual pixel
  masks are applied before the image is embedded into the output PDF.
- Requires an external PDF renderer: `pdftoppm` (poppler-utils), `mutool`
  (mupdf-tools), or `gs` (ghostscript).
- Without a renderer, the command fails with a clear error message.
- Intermediate PPM files are written to `target/pdf-cleanroom-flatten/`
  (gitignored).
- Manual pixel masks use rendered image pixel coordinates with a top-left
  origin and are clipped to image bounds. They are currently exposed through
  internal APIs/tests, not a production redaction CLI.


### Structural verification

```sh
# No extractable PDF text should be emitted.
pdftotext output.pdf -

# At least one page image should be listed.
pdfimages -list output.pdf

# Inspect object structure: expect /Subtype /Image and no /Font, /Annots,
# /AcroForm, /Metadata, or /EmbeddedFiles from the source PDF.
qpdf --json=2 output.pdf

# Optional low-level operator check: no text objects or text-show operators.
qpdf --qdf --object-streams=disable output.pdf output.qdf.pdf
grep -aE 'BT|ET|Tf|Tj|TJ' output.qdf.pdf
```

### Flatten smoke tests

```sh
PDF_CLEANROOM_FLATTEN_TESTS=1 cargo test --test integration_flatten
```

Generates a synthetic PDF with known secrets, flattens it, and verifies:
- Output is a valid PDF with non-trivial size
- Normal text extraction returns no text (image-only)
- Output contains at least one image XObject and no Font objects
- Obvious source secret bytes are not copied into the output PDF
- Manual pixel masks mutate the embedded raster image before PDF rebuild
- No mandatory dependencies added — normal `cargo test` is unaffected

### Limitations

- **Manual pixel masks only.** `flatten-raster` can black out explicit rendered
  image pixel rectangles before embedding, but it does not yet detect OCR boxes
  or PDF text bounding boxes automatically.
- **No OCR.** The output has no hidden text layer.
- **Renderer quality.** Visual fidelity depends on the external renderer's
  DPI and capabilities. 200 DPI is the default.
- **Page dimensions.** Output page dimensions approximate the source via
  MediaBox extraction or image dimensions at render DPI.

See [DESIGN.md](DESIGN.md) and [BENCHMARKS.md](BENCHMARKS.md) for the
longer-term strategy around visual fidelity measurement and OCR integration.

## License

MIT
