# Agent Study: pdf-cleanroom

This project is both a working PDF sanitization tool and a documented study of
LLM-assisted coding applied to a security-sensitive domain.

## Motivation

Every line of production code, test, and design decision was developed through
conversational LLM interaction — a human operator providing goals and safety
constraints, AI generating code and tests. The project explores:

- Whether LLM-generated code can handle **security-sensitive domains** (secret
  detection, PDF sanitization) where bugs have real consequences.
- How to produce **defensible, test-covered code** rather than plausible stubs.
- Whether **negative safety constraints** ("never copy source PDF structure",
  "never simulate redaction by overlay") remain enforceable across iterations.
- How to generate **hostile test PDFs at runtime** (lopdf/printpdf) instead of
  committing binary fixtures.

## Workflow

Human → high-level goals + domain safety rules. AI → implementation, tests,
regression fixes. Human verified each phase by running tests and reviewing
output. Design conversations are recorded in [PROJECT_LOG.md](PROJECT_LOG.md)
and [DESIGN.md](DESIGN.md).

## Key Findings

1. **Dynamic test fixtures work well.** All test PDFs are built at runtime by
   helper functions — no binary blobs committed.
2. **Safety invariants can be test-asserted.** Raw-byte regression checks
   (`assert_no_secret_bytes`) ensure rebuilt PDFs do not accidentally leak
   source secrets.
3. **Negative constraints are enforceable** when stated early and checked by
   tests the AI also writes.
4. **Explicit pruning passes are necessary.** The AI naturally expands —
   trimming unused code and tightening documentation required deliberate
   follow-up.

## Limitations

- The AI cannot test against real-world PDFs it hasn't been told about.
- OCR and image-preserving flatten-visible are correctly scoped out but remain
   unimplemented.
- Coverage matches what the prompt specified; adversarial exploration beyond
   the prompt was minimal.

## See Also

- [DESIGN.md](DESIGN.md) — architecture, library choices, test strategy
- [PROJECT_LOG.md](PROJECT_LOG.md) — chronological development log
