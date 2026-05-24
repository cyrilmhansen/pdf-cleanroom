# Visual Fidelity Benchmarks

pdf-cleanroom has several output families with very different fidelity profiles:

| Strategy | Fidelity goal | Likely fidelity |
|---|---|---|
| `text-only` | Clean text archival and AI ingestion | Low visual fidelity |
| `flatten-raster` | Image-only visual flattening | High visual fidelity, no text layer |
| `raster-redacted` | Image-only visual redaction via pixel masks | High outside masks, intentionally different inside masks |
| `layered-pdf` | DjVu-like searchable visual clean-room PDF | High visual fidelity plus sanitized derived search text |
| `preserve` | Future destructive original-structure redaction | Unknown (not implemented) |

A benchmark is needed to quantify these trade-offs, both for regression prevention
and to guide future strategy development.

## Benchmark workflow

```
input.pdf
  │
  ├─── render ──→ input-page-1.png        (reference)
  │
  ├─── rebuild/preserve ──→ output.pdf
  │       │
  │       ├─── render ──→ output-page-1.png
  │       │
  │       ├─── structural inspect ──→ report (qpdf)
  │       │
  │       └─── text extract ──→ text (lopdf)
  │
  └─── compare input vs output:
          ├── pixel metrics (PSNR, SSIM, phash)
          ├── structural metrics (annotations, forms, attachments)
          ├── text usability (OCR anchor recall — optional)
          └── sanitization invariants (secret leakage)
```

## Metrics

### Image-based

All metrics are computed per-page at a fixed DPI (default 150 DPI, configurable).

| Metric | Symbol | Range | Meaning |
|---|---|---|---|
| **Dimensions** | `width × height` | pixels | Rendered page size; mismatch flags scaling or cropping differences |
| **Mean absolute pixel error** | MAE | `[0, 255]` | Average absolute channel difference. Lower is better. |
| **Peak signal-to-noise ratio** | PSNR | `[0, ∞)` dB | Log-scale fidelity. Higher is better. >40 dB ≈ imperceptible loss. |
| **Structural similarity index** | SSIM | `[0, 1]` | Perceived similarity including luminance, contrast, structure. ≥0.95 ≈ identical. |
| **Simplified SSIM (s-SSIM)** | s-SSIM | `[0, 1]` | Lightweight SSIM using block statistics without full covariance matrix. |
| **Perceptual hash similarity** | phash | `[0, 1]` | hamming-distance-based similarity. Useful for quick duplicate detection. |

### Redaction-aware comparison

If the output strategy intentionally masks a secret region, the benchmark must
not penalise that region as a visual difference. The comparison algorithm:

1. Render both input and output pages to PNG at the same DPI.
2. Align them (crop/scale if needed; warn if sizes differ).
3. Compare **unmasked regions** for visual fidelity: MAE should stay low, SSIM
   should stay high, and any phash/PSNR regression should be investigated.
4. Compare **masked regions** separately: they should differ strongly from the
   input and should be dominated by the expected mask color/style.
5. Run OCR checks separately on output renders: non-secret anchors should remain
   readable, while masked secret OCR hits should be zero.

Mask regions can be:
- **Manual**: explicit `MaskRegion { page, x, y, width, height, source, reason }`
  rectangles in PDF points, converted to rendered pixels.
- **Detector-derived**: future areas mapped from sanitized text extraction or
  OCR findings.
- **Provided externally** via a bounding-box list (e.g. from qpdf inspection,
  annotation rects, or a future preserve backend's redaction list).
- **Fallback**: if no mask regions are known, the whole page is compared
  unmasked (and metrics are expected to be poor for `text-only`).

### File size

| Metric | Meaning |
|---|---|
| `input_size_bytes` | Source PDF size |
| `output_size_bytes` | Result PDF size |
| `size_ratio` | `output / input`; <1 means smaller |

### Runtime

| Metric | Meaning |
|---|---|
| `render_ms` | Time to render all pages |
| `process_ms` | Time for rebuild/preserve logic |
| `compare_ms` | Time to compute pixel metrics |

### Sanitization invariants (structural)

Performed by `qpdf --json=2` on the output PDF. All must pass:

| Check | Condition |
|---|---|
| no annotations | `pages[*].annotations` empty |
| no attachments | `attachments` empty |
| no AcroForm | `acroform.hasacroform == false` |
| no source metadata | Info dict only contains pdf-cleanroom metadata |
| no source XMP | Metadata stream absent or sanitised |
| no encryption | `encrypt.encrypted == false` |
| no embedded scripts | No `/JS` or `/JavaScript` in object tree |

### Sanitization invariants (text)

Performed by `lopdf` extraction + regex scan on output text:

| Check | Condition |
|---|---|
| no emails | No email pattern matches |
| no phone numbers | No French phone pattern matches |
| no IBANs | No IBAN pattern matches |
| no raw secret bytes | Original secret strings absent from raw `.pdf` bytes |

### OCR anchor recall (future, optional)

If a Tesseract backend is available, extract text from **output page PNG** and
check that non-secret text anchors (headings, labels, amounts) are still
readable. This is the "text usability" metric.

| Metric | Meaning |
|---|---|
| `ocr_anchor_recall` | Fraction of known non-secret strings detected by OCR in output |
| `secret_ocr_hits` | Count of known secret strings detected by OCR in output; must be zero for redacted outputs |

## Benchmark output format

Each run produces a dated directory under `target/pdf-cleanroom-bench/`:

```
target/pdf-cleanroom-bench/
├── 2026-05-24T18:00:00Z/
│   ├── input.pdf
│   ├── output.pdf
│   ├── config.json              # strategy, mask mode, DPI, DUT name
│   ├── renders/
│   │   ├── input-page-1.png
│   │   ├── output-page-1.png
│   │   └── diff-page-1.png      # per-pixel difference heatmap
│   ├── metrics.json             # machine-readable results
│   └── report.md                # human-readable summary
```

### `metrics.json` schema

```jsonc
{
  "benchmark_version": 1,
  "timestamp": "2026-05-24T18:00:00Z",
  "config": {
    "strategy": "text-only",
    "mask_mode": "label",
    "dpi": 150,
    "dut": "pdf-cleanroom 0.1.0"
  },
  "pages": [
    {
      "page": 1,
      "width_px": 1240,
      "height_px": 1754,
      "size_ratio": 0.3,
      "render_ms": 45,
      "process_ms": 12,
      "compare_ms": 80,
      "unmasked": {
        "mae": 2.1,
        "psnr": 42.3,
        "ssim": 0.987,
        "s_ssim": 0.983,
        "phash_similarity": 0.95
      },
      "masked": {
        "mae": 78.3,
        "psnr": 8.2,
        "ssim": 0.12,
        "s_ssim": 0.15,
        "phash_similarity": 0.32,
        "mask_region_count": 2
      },
      "sanitization": {
        "secrets_found_input": 4,
        "secrets_found_output": 0,
        "raw_byte_leak": false,
        "annotations_present": false,
        "attachments_present": false,
        "acroform_present": false,
        "encrypted": false
      },
      "ocr_anchor_recall": null,
      "secret_ocr_hits": null
    }
  ],
  "summary": {
    "total_pages": 1,
    "mean_unmasked_psnr": 42.3,
    "mean_unmasked_ssim": 0.987,
    "mean_masked_mae": 78.3,
    "size_ratio": 0.3,
    "total_render_ms": 45,
    "total_process_ms": 12,
    "total_compare_ms": 80,
    "all_sanitization_checks_passed": true
  }
}
```

## Implementation plan (future runs)

1. **Phase 1 — Skeleton** (this run): structs, documentation, no rendering.
2. **Phase 2 — Render backends**: integrate `pdftoppm` / `mutool draw` /
   Ghostscript as optional external renderers invoked via `std::process::Command`.
3. **Phase 3 — Pixel comparison**: implement PSNR, SSIM, MAE, phash on
   rendered PNGs using the `image` crate (optional feature gate).
4. **Phase 4 — Redaction-aware masking**: auto-detect mask regions from
   text extraction + secret positions.
5. **Phase 5 — Integration**: wire benchmark into CI, produce regressions
   reports.

## Renderer candidates

| Tool | Availability | Quality | Speed | License | Status |
|---|---|---|---|---|---|
| `pdftoppm` (poppler) | Common on Linux/macOS | High | Fast | GPL-2.0 | Preferred, verified working |
| `mutool draw` (MuPDF) | Common on Linux/macOS | High | Fast | AGPL-3.0 | Fallback |
| `gs` (Ghostscript) | Common on Linux/macOS | High | Medium | AGPL-3.0 | Fallback |
| `pdfium-render` crate | Cargo dependency | High | Fast | MIT/Apache-2.0 | Future optional crate |

All renderers are invoked via `std::process::Command` — no native library
linking required. The benchmark detects which (if any) are installed and
selects the first available in priority order: `pdftoppm > mutool > gs`.

## Ghostscript rendering command

```sh
gs -dNOPAUSE -dBATCH -sDEVICE=png16m -r150 \
   -sOutputFile=output-page-%d.png input.pdf
```

## See also

- [PRESERVE_BACKENDS.md](./PRESERVE_BACKENDS.md) — tool roles and backend candidates
- [DESIGN.md](./DESIGN.md) — output strategy rationale
