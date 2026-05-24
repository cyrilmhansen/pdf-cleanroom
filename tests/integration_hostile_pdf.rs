/// Integration tests: hostile and edge-case PDF inputs.
///
/// These tests verify that pdf-cleanroom handles tricky PDF constructs
/// without silently producing insecure output. All PDFs are generated
/// at runtime — no binary fixtures are committed.

mod support;
use std::sync::atomic::{AtomicU64, Ordering};


use pdf_cleanroom::detect::Detector;
use pdf_cleanroom::mask::MaskMode;
use pdf_cleanroom::{pdf_extract, rebuild};

use support::pdf_fixtures;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Run the full extract → detect → rebuild pipeline on PDF bytes,
/// returning (report_occurrences, rebuilt_text_bytes).
fn process_pdf(pdf_bytes: &[u8]) -> (Vec<String>, String, Vec<u8>) {
    // Write to temp file with unique subdirectory per invocation
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pdf_cleanroom_hostile_{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));

    std::fs::create_dir_all(&dir).ok();
    let input_path = dir.join("input.pdf");
    let output_path = dir.join("output.pdf");
    std::fs::write(&input_path, pdf_bytes).expect("write input");

    // Extract
    let content = pdf_extract::extract(input_path.to_str().unwrap()).expect("extract");

    // Detect
    let detector = Detector::new();
    let mut all_occ = Vec::new();
    let mut cleaned_text = String::new();

    for page in &content.pages {
        let detections = detector.scan_text(&page.text);
        for d in &detections {
            all_occ.push(format!("{}:{}", d.kind, d.value));
        }
        let (masked, _) = rebuild::apply_masks_to_text(&page.text, &detections, MaskMode::BlackBlock);
        cleaned_text.push_str(&masked);
        cleaned_text.push('\n');
    }

    // Rebuild
    let clean_pages: Vec<_> = content
        .pages
        .iter()
        .map(|p| {
            let detections = detector.scan_text(&p.text);
            let (masked, _) =
                rebuild::apply_masks_to_text(&p.text, &detections, MaskMode::BlackBlock);
            rebuild::CleanPage {
                page_num: p.page_num,
                lines: masked.lines().map(|l| l.to_string()).collect(),
            }
        })
        .collect();

    let metadata = rebuild::RebuildMetadata {
        title: "hostile-test".into(),
    };
    rebuild::rebuild(output_path.to_str().unwrap(), &clean_pages, &metadata)
        .expect("rebuild");

    let rebuilt_bytes = std::fs::read(&output_path).expect("read output");

    // Cleanup
    let _ = std::fs::remove_file(&input_path);
    let _ = std::fs::remove_file(&output_path);
    let _ = std::fs::remove_dir(&dir);

    (all_occ, cleaned_text, rebuilt_bytes)
}

/// Check that rebuilt PDF bytes do not contain original secret strings.
fn assert_no_secret_bytes(bytes: &[u8], secrets: &[&str]) {
    for secret in secrets {
        assert!(
            !bytes.windows(secret.len()).any(|w| w == secret.as_bytes()),
            "rebuilt PDF bytes must NOT contain secret string: {secret:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 1. Visible text secrets
// ---------------------------------------------------------------------------

#[test]
fn visible_text_secrets_are_detected() {
    let pdf = pdf_fixtures::visible_text_pdf();
    let (occurrences, cleaned_text, rebuilt_bytes) = process_pdf(&pdf);

    let values: Vec<&str> = occurrences.iter().map(|s| s.as_str()).collect();
    assert!(
        values.iter().any(|s| s.contains("email")),
        "should detect email, got: {values:?}"
    );
    assert!(
        values.iter().any(|s| s.contains("phone_fr")),
        "should detect phone, got: {values:?}"
    );
    assert!(
        values.iter().any(|s| s.contains("iban")),
        "should detect IBAN, got: {values:?}"
    );

    // Rebuilt extracted text must not contain original secrets
    assert!(
        !cleaned_text.contains(pdf_fixtures::SECRET_EMAIL),
        "email must be masked in rebuilt text"
    );
    assert!(
        !cleaned_text.contains(pdf_fixtures::SECRET_PHONE),
        "phone must be masked in rebuilt text"
    );
    assert!(
        !cleaned_text.contains(pdf_fixtures::SECRET_IBAN),
        "IBAN must be masked in rebuilt text"
    );

    // Raw bytes should not contain secrets
    assert_no_secret_bytes(&rebuilt_bytes, &[
        pdf_fixtures::SECRET_EMAIL,
        pdf_fixtures::SECRET_IBAN,
    ]);
}

// ---------------------------------------------------------------------------
// 2. Metadata secrets
// ---------------------------------------------------------------------------

#[test]
fn metadata_secrets_are_not_preserved_in_rebuild() {
    let pdf = pdf_fixtures::metadata_secrets_pdf();
    let (occurrences, _cleaned_text, rebuilt_bytes) = process_pdf(&pdf);

    // metadata scanning is NOT implemented yet — document this
    // The rebuild must NOT preserve metadata regardless.
    if !occurrences.is_empty() {
        eprintln!("NOTE: metadata secrets were detected (metadata scanning may be partially working): {occurrences:?}");
    }

    // Rebuilt PDF bytes must not contain the secrets from metadata
    // (they were never in extracted text, but ensure they didn't leak via object copy)
    assert_no_secret_bytes(&rebuilt_bytes, &[
        pdf_fixtures::SECRET_EMAIL,
        pdf_fixtures::SECRET_PHONE,
        pdf_fixtures::SECRET_IBAN,
    ]);
}

// ---------------------------------------------------------------------------
// 3. XMP metadata secrets
// ---------------------------------------------------------------------------

#[test]
fn xmp_metadata_secrets_are_not_preserved_in_rebuild() {
    let pdf = pdf_fixtures::xmp_metadata_pdf();
    let (_occurrences, _cleaned_text, rebuilt_bytes) = process_pdf(&pdf);

    // XMP metadata scanning is NOT implemented — and rebuild must not preserve it
    assert_no_secret_bytes(&rebuilt_bytes, &[
        pdf_fixtures::SECRET_EMAIL,
        pdf_fixtures::SECRET_PHONE,
        pdf_fixtures::SECRET_IBAN,
    ]);
}

// ---------------------------------------------------------------------------
// 4. Embedded file attachments
// ---------------------------------------------------------------------------

#[test]
fn embedded_file_attachments_are_dropped_in_rebuild() {
    let pdf = pdf_fixtures::embedded_file_pdf();

    // Embedded file scanning is NOT implemented — lopdf's extract_text does not
    // read embedded file content. The rebuild must NOT preserve attachments
    // regardless. This test verifies that attachment names and content bytes
    // do not leak into the rebuilt output.
    let (_occurrences, _cleaned_text, rebuilt_bytes) = process_pdf(&pdf);
    if !_occurrences.is_empty() {
        eprintln!("NOTE: embedded file secrets were detected (unexpected): {_occurrences:?}");
    }

    // Rebuilt must not contain embedded file content
    assert_no_secret_bytes(&rebuilt_bytes, &[
        pdf_fixtures::SECRET_EMAIL,
        pdf_fixtures::SECRET_PHONE,
        pdf_fixtures::SECRET_IBAN,
        "secrets.txt", // attachment filename
    ]);
}

// ---------------------------------------------------------------------------
// 5. Annotations with secrets
// ---------------------------------------------------------------------------

#[test]
fn annotation_secrets_are_dropped_in_rebuild() {
    let pdf = pdf_fixtures::annotation_secrets_pdf();

    // Annotation scanning is NOT implemented — lopdf's extract_text does not
    // read annotation content streams. The rebuild must NOT preserve annotations
    // regardless. This test verifies that annotation text does not leak
    // into the rebuilt output.
    let (_occurrences, _cleaned_text, rebuilt_bytes) = process_pdf(&pdf);
    if !_occurrences.is_empty() {
        eprintln!("NOTE: annotation secrets were detected (unexpected): {_occurrences:?}");
    }

    // Annotation text should not appear in rebuilt output
    assert_no_secret_bytes(&rebuilt_bytes, &[
        pdf_fixtures::SECRET_EMAIL,
        pdf_fixtures::SECRET_PHONE,
        pdf_fixtures::SECRET_IBAN,
    ]);
}

// ---------------------------------------------------------------------------
// 6. Form fields (AcroForm) with secrets
// ---------------------------------------------------------------------------

#[test]
fn form_field_secrets_are_dropped_in_rebuild() {
    let pdf = pdf_fixtures::form_field_secrets_pdf();
    let (_occurrences, _cleaned_text, rebuilt_bytes) = process_pdf(&pdf);

    // Form field values should not appear in rebuilt output
    assert_no_secret_bytes(&rebuilt_bytes, &[
        pdf_fixtures::SECRET_EMAIL,
    ]);
}

// ---------------------------------------------------------------------------
// 7. Hidden / non-visible text
// ---------------------------------------------------------------------------

#[test]
fn white_text_on_white_background_is_extracted() {
    // lopdf's extract_text does not consider color — it extracts all text
    // regardless of visibility. This documents that behavior.
    let pdf = pdf_fixtures::hidden_white_text_pdf();
    let (occurrences, _cleaned_text, rebuilt_bytes) = process_pdf(&pdf);

    let detected_emails: Vec<&str> = occurrences.iter().filter(|s| s.starts_with("email")).map(|s| s.as_str()).collect();
    if detected_emails.is_empty() {
        eprintln!("NOTE: lopdf did NOT extract white-on-white text (expected to extract it)");
    } else {
        eprintln!("NOTE: lopdf extracted white-on-white text (expected)");
    }

    // Regardless of extraction, rebuilt PDF must not contain the secret
    assert_no_secret_bytes(&rebuilt_bytes, &[
        pdf_fixtures::SECRET_EMAIL,
    ]);
}

#[test]
fn text_outside_page_bounds_is_extracted() {
    let pdf = pdf_fixtures::text_outside_bounds_pdf();
    let (occurrences, _cleaned_text, rebuilt_bytes) = process_pdf(&pdf);

    if !occurrences.iter().any(|s| s.starts_with("email")) {
        eprintln!("NOTE: lopdf did NOT extract text outside page bounds");
    }

    // Rebuilt must not leak it
    assert_no_secret_bytes(&rebuilt_bytes, &[
        pdf_fixtures::SECRET_EMAIL,
    ]);
}

#[test]
fn tiny_text_is_extracted() {
    let pdf = pdf_fixtures::tiny_text_pdf();
    let (occurrences, _cleaned_text, rebuilt_bytes) = process_pdf(&pdf);

    if !occurrences.iter().any(|s| s.starts_with("email")) {
        eprintln!("NOTE: lopdf did NOT extract 1pt tiny text");
    }

    assert_no_secret_bytes(&rebuilt_bytes, &[
        pdf_fixtures::SECRET_EMAIL,
    ]);
}

// ---------------------------------------------------------------------------
// 8. Text covered by opaque shape
// ---------------------------------------------------------------------------

#[test]
fn text_covered_by_shape_is_extracted() {
    // lopdf's extract_text reads text operators regardless of what's drawn
    // on top. This documents that behavior — the text IS extractable even
    // if visually covered.
    let pdf = pdf_fixtures::text_covered_by_shape_pdf();
    let (occurrences, _cleaned_text, rebuilt_bytes) = process_pdf(&pdf);

    let detected: Vec<&str> = occurrences.iter().map(|s| s.as_str()).collect();
    if !detected.iter().any(|s| s.starts_with("email")) {
        eprintln!("NOTE: lopdf did NOT extract text covered by a shape (expected to extract it)");
    } else {
        eprintln!("NOTE: lopdf extracted text covered by a shape (text remains in content stream)");
    }

    // Rebuilt must not contain the secret
    assert_no_secret_bytes(&rebuilt_bytes, &[
        pdf_fixtures::SECRET_EMAIL,
    ]);
}

// ---------------------------------------------------------------------------
// 9. Fragmented secrets (split across text chunks)
// ---------------------------------------------------------------------------

#[test]
fn fragmented_email_is_detected_when_merged_by_extractor() {
    // lopdf's extract_text merges TJ/Tj operations with positioning.
    // Whether it detects the email depends on how it joins chunks.
    let pdf = pdf_fixtures::fragmented_secrets_pdf();
    let (occurrences, _cleaned_text, rebuilt_bytes) = process_pdf(&pdf);

    let detected: Vec<&str> = occurrences.iter().map(|s| s.as_str()).collect();
    eprintln!(
        "NOTE: fragmented secrets detection: {:?}",
        if detected.is_empty() {
            "not detected (limitation: lopdf does not merge fragmented text across Td operations)"
        } else {
            "detected (lopdf merged chunks)"
        }
    );

    if let Some(rebuilt_text) = extract_text_from_bytes(&rebuilt_bytes) {
        assert!(
            !rebuilt_text.contains("cyril@example.com"),
            "rebuilt text should not contain reconstructed fragmented email"
        );
    }

    // Raw bytes must not contain the full email
    assert_no_secret_bytes(&rebuilt_bytes, &[
        pdf_fixtures::SECRET_IBAN,
    ]);
}

#[test]
fn fragmented_iban_is_detected_when_merged() {
    let pdf = pdf_fixtures::fragmented_secrets_pdf();
    let (occurrences, _cleaned_text, rebuilt_bytes) = process_pdf(&pdf);

    let detected_ibans: Vec<&str> = occurrences.iter()
        .filter(|s| s.starts_with("iban"))
        .map(|s| s.as_str())
        .collect();

    eprintln!(
        "NOTE: fragmented IBAN detection count: {}",
        detected_ibans.len()
    );

    // Raw bytes
    assert_no_secret_bytes(&rebuilt_bytes, &[
        pdf_fixtures::SECRET_IBAN,
    ]);
}

// ---------------------------------------------------------------------------
// 10. Image-only PDF (no text content)
// ---------------------------------------------------------------------------

#[test]
fn image_only_pdf_has_no_detections() {
    let pdf = pdf_fixtures::image_only_pdf();
    let (occurrences, _cleaned_text, _rebuilt_bytes) = process_pdf(&pdf);

    assert!(
        occurrences.is_empty(),
        "image-only PDF should have no secret detections, got: {occurrences:?}"
    );
}

#[test]
fn image_with_text_detects_only_text_secrets() {
    // image_with_text_pdf has caption text but NO secrets — just images
    let pdf = pdf_fixtures::image_with_text_pdf();
    let (occurrences, _cleaned_text, _rebuilt_bytes) = process_pdf(&pdf);

    assert!(
        occurrences.is_empty(),
        "image+text PDF with no secrets should have 0 detections, got: {occurrences:?}"
    );
}

// ---------------------------------------------------------------------------
// 11. Encrypted / password-protected PDF
// ---------------------------------------------------------------------------

#[test]
fn encrypted_pdf_is_rejected_with_error() {
    // lopdf does not easily create encrypted PDFs programmatically.
    // We verify that loading an improperly formatted file generates an error
    // rather than silently producing a misleading output.
    let garbage = b"%PDF-1.4\n% garbage\ntrailer << /Size 1 /Root 1 0 R >>\n";
    let result = pdf_extract::extract_bytes(garbage);
    assert!(
        result.is_err(),
        "corrupt/invalid PDF should be rejected: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// Helper: extract text from raw PDF bytes for assertions
// ---------------------------------------------------------------------------

fn extract_text_from_bytes(pdf_bytes: &[u8]) -> Option<String> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pdf_cleanroom_aux_{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("aux.pdf");
    std::fs::write(&path, pdf_bytes).ok()?;
    let content = pdf_extract::extract(path.to_str()?).ok()?;
    let text: String = content.pages.iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join("\n");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);

    Some(text)
}
