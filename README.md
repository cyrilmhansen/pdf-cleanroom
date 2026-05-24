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
pdf-cleanroom rebuild input.pdf output.pdf --report report.json
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

## License

MIT
