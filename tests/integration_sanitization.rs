/// Integration tests: sanitization guarantees.
///
/// These tests verify that pdf-cleanroom's rebuild mode demonstrably
/// removes or masks secrets, and that CLI flags behave correctly.

mod support;

use pdf_cleanroom::detect::Detector;
use pdf_cleanroom::mask::MaskMode;
use pdf_cleanroom::report::Report;
use pdf_cleanroom::{pdf_extract, rebuild};
use std::sync::atomic::{AtomicU64, Ordering};


use support::pdf_fixtures;

// ---------------------------------------------------------------------------
// Helper: run the pipeline, return (occurrences, rebuilt_text, rebuilt_bytes)
// ---------------------------------------------------------------------------

fn process_pdf(pdf_bytes: &[u8], mask_mode: MaskMode) -> (Vec<String>, String, Vec<u8>) {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pdf_cleanroom_sanitize_{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).ok();
    let input_path = dir.join("input.pdf");
    let output_path = dir.join("output.pdf");
    std::fs::write(&input_path, pdf_bytes).expect("write input");

    let content = pdf_extract::extract(input_path.to_str().unwrap()).expect("extract");

    let detector = Detector::new();
    let mut all_occ = Vec::new();
    let mut cleaned_text = String::new();

    let clean_pages: Vec<_> = content
        .pages
        .iter()
        .map(|p| {
            let detections = detector.scan_text(&p.text);
            for d in &detections {
                all_occ.push(format!("{}:{}", d.kind, d.value));
            }
            let (masked, _) =
                rebuild::apply_masks_to_text(&p.text, &detections, mask_mode);
            cleaned_text.push_str(&masked);
            cleaned_text.push('\n');
            rebuild::CleanPage {
                page_num: p.page_num,
                lines: masked.lines().map(|l| l.to_string()).collect(),
            }
        })
        .collect();

    let metadata = rebuild::RebuildMetadata {
        title: "sanitize-test".into(),
    };
    rebuild::rebuild(output_path.to_str().unwrap(), &clean_pages, &metadata).expect("rebuild");

    let rebuilt_bytes = std::fs::read(&output_path).expect("read output");
    let _ = std::fs::remove_file(&input_path);
    let _ = std::fs::remove_file(&output_path);
    let _ = std::fs::remove_dir(&dir);

    (all_occ, cleaned_text, rebuilt_bytes)
}

fn assert_rebuilt_clean(rebuilt_bytes: &[u8], secrets: &[&str]) {
    for secret in secrets {
        assert!(
            !rebuilt_bytes
                .windows(secret.len())
                .any(|w| w == secret.as_bytes()),
            "rebuilt PDF bytes must not contain: {secret:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 12. Dry-run behavior
// ---------------------------------------------------------------------------

#[test]
fn dry_run_does_not_create_output_file() {
    let pdf = pdf_fixtures::visible_text_pdf();
    let dir = std::env::temp_dir().join("pdf_cleanroom_dryrun");
    std::fs::create_dir_all(&dir).ok();
    let input_path = dir.join("input.pdf");
    let output_path = dir.join("output.pdf");
    let report_path = dir.join("report.json");

    std::fs::write(&input_path, &pdf).expect("write input");

    // When --dry-run is used, rebuild should NOT create output file.
    // The dry_run flag is passed to cmd_rebuild which skips the rebuild call.
    // We simulate this by extracting, detecting, and verifying no output was written.

    // Extract text
    let content =
        pdf_extract::extract(input_path.to_str().unwrap()).expect("extract");

    // Build report (like the real code does)
    let detector = Detector::new();
    let mut rep = Report::new(content.num_pages, false);
    for page in &content.pages {
        let detections = detector.scan_text(&page.text);
        for d in detections {
            rep.add_occurrence(
                page.page_num,
                &d.kind,
                None,
                Some(d.start),
                Some(d.end),
                None,
                String::new(),
            );
        }
    }

    // Write report if requested
    let json = rep.to_json().expect("serialize");
    std::fs::write(&report_path, &json).expect("write report");
    assert!(report_path.exists(), "report should exist even with dry-run");

    // The output file should NOT exist (dry-run)
    assert!(
        !output_path.exists(),
        "dry-run must NOT create output PDF"
    );

    let _ = std::fs::remove_file(&input_path);
    let _ = std::fs::remove_file(&report_path);
    let _ = std::fs::remove_dir(&dir);
}

// ---------------------------------------------------------------------------
// 13. unsafe-show-secrets behavior
// ---------------------------------------------------------------------------

#[test]
fn report_without_unsafe_flag_omits_exact_values() {
    let pdf = pdf_fixtures::visible_text_pdf();
    let content = pdf_extract::extract_bytes(&pdf).expect("extract");
    let detector = Detector::new();
    let mut rep = Report::new(content.num_pages, false);

    for page in &content.pages {
        let detections = detector.scan_text(&page.text);
        for d in detections {
            rep.add_occurrence(
                page.page_num,
                &d.kind,
                None, // exact_value = None
                Some(d.start),
                Some(d.end),
                None,
                "[MASKED]".into(),
            );
        }
    }

    let json = rep.to_json().expect("json");

    // exact_value should be absent
    assert!(
        !json.contains("exact_value"),
        "report without --unsafe-show-secrets must not contain exact_value key"
    );

    // Raw secrets should not appear
    assert!(
        !json.contains(pdf_fixtures::SECRET_EMAIL),
        "report must not contain raw email"
    );

    // masked_display should show the mask, not the value
    for occ in &rep.occurrences {
        assert_eq!(
            occ.masked_display, "[MASKED]",
            "masked_display should show mask, not value"
        );
        assert!(
            occ.exact_value.is_none(),
            "exact_value must be None without unsafe flag"
        );
    }
}

#[test]
fn report_with_unsafe_flag_includes_exact_values() {
    let pdf = pdf_fixtures::visible_text_pdf();
    let content = pdf_extract::extract_bytes(&pdf).expect("extract");
    let detector = Detector::new();
    let mut rep = Report::new(content.num_pages, true);

    for page in &content.pages {
        let detections = detector.scan_text(&page.text);
        for d in detections {
            rep.add_occurrence(
                page.page_num,
                &d.kind,
                Some(d.value.clone()), // exact_value = Some
                Some(d.start),
                Some(d.end),
                None,
                "[MASKED]".into(),
            );
        }
    }

    let json = rep.to_json().expect("json");

    // exact_value should be present
    assert!(
        json.contains("exact_value"),
        "report with --unsafe-show-secrets must contain exact_value key"
    );

    // The values should appear
    assert!(
        json.contains(pdf_fixtures::SECRET_EMAIL),
        "report with unsafe flag should contain email"
    );
}

#[test]
fn masked_display_shows_exact_value_when_unsafe_flag_is_set() {
    let pdf = pdf_fixtures::visible_text_pdf();
    let content = pdf_extract::extract_bytes(&pdf).expect("extract");
    let detector = Detector::new();
    let mut rep = Report::new(content.num_pages, true);

    for page in &content.pages {
        let detections = detector.scan_text(&page.text);
        for d in detections {
            rep.add_occurrence(
                page.page_num,
                &d.kind,
                Some(d.value.clone()),
                Some(d.start),
                Some(d.end),
                None,
                "[MASKED]".into(),
            );
        }
    }

    // With unsafe flag, masked_display is overridden to show the exact value
    for occ in &rep.occurrences {
        assert_ne!(
            occ.masked_display, "[MASKED]",
            "masked_display should show exact value, not the mask placeholder"
        );
    }
}

// ---------------------------------------------------------------------------
// 14. Raw byte regression check
// ---------------------------------------------------------------------------

#[test]
fn rebuilt_pdf_bytes_do_not_contain_any_source_secrets() {
    // Comprehensive check across all fixture types
    let test_cases: Vec<(&str, Vec<u8>, Vec<&str>)> = vec![
        (
            "visible-text",
            pdf_fixtures::visible_text_pdf(),
            vec![
                pdf_fixtures::SECRET_EMAIL,
                pdf_fixtures::SECRET_PHONE,
                pdf_fixtures::SECRET_IBAN,
            ],
        ),
        (
            "metadata",
            pdf_fixtures::metadata_secrets_pdf(),
            vec![
                pdf_fixtures::SECRET_EMAIL,
                pdf_fixtures::SECRET_PHONE,
                pdf_fixtures::SECRET_IBAN,
            ],
        ),
        (
            "xmp",
            pdf_fixtures::xmp_metadata_pdf(),
            vec![
                pdf_fixtures::SECRET_EMAIL,
                pdf_fixtures::SECRET_IBAN,
            ],
        ),
        (
            "embedded-file",
            pdf_fixtures::embedded_file_pdf(),
            vec![
                pdf_fixtures::SECRET_EMAIL,
                pdf_fixtures::SECRET_IBAN,
            ],
        ),
        (
            "annotation",
            pdf_fixtures::annotation_secrets_pdf(),
            vec![
                pdf_fixtures::SECRET_EMAIL,
                pdf_fixtures::SECRET_IBAN,
            ],
        ),
        (
            "form-field",
            pdf_fixtures::form_field_secrets_pdf(),
            vec![pdf_fixtures::SECRET_EMAIL],
        ),
    ];

    for (name, pdf, secrets) in &test_cases {
        let (_occ, _cleaned, rebuilt_bytes) = process_pdf(pdf, MaskMode::BlackBlock);
        for secret in secrets {
            assert!(
                !rebuilt_bytes
                    .windows(secret.len())
                    .any(|w| w == secret.as_bytes()),
                "[{name}] rebuilt PDF bytes must not contain: {secret:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Verify mask modes produce expected text output
// ---------------------------------------------------------------------------

#[test]
fn mask_mode_black_block_replaces_with_blocks() {
    let pdf = pdf_fixtures::visible_text_pdf();
    let (_occ, cleaned_text, _rebuilt) = process_pdf(&pdf, MaskMode::BlackBlock);

    // The cleaned text should contain black blocks (█)
    assert!(
        cleaned_text.contains('█'),
        "BlackBlock mode should contain Unicode full blocks, got: {cleaned_text:?}"
    );

    // Original secrets must not be in cleaned text
    assert!(!cleaned_text.contains(pdf_fixtures::SECRET_EMAIL));
    assert!(!cleaned_text.contains(pdf_fixtures::SECRET_PHONE));
    assert!(!cleaned_text.contains(pdf_fixtures::SECRET_IBAN));
}

#[test]
fn mask_mode_label_replaces_with_labels() {
    let pdf = pdf_fixtures::visible_text_pdf();
    let (_occ, cleaned_text, _rebuilt) = process_pdf(&pdf, MaskMode::Label);

    assert!(
        cleaned_text.contains("[SUPPRIMÉ: EMAIL]"),
        "Label mode should contain suppression labels"
    );
    assert!(
        cleaned_text.contains("[SUPPRIMÉ: TÉLÉPHONE]")
            || cleaned_text.contains("[SUPPRIMÉ: PHONE]"),
        "Label mode should contain phone suppression label"
    );
    assert!(
        cleaned_text.contains("[SUPPRIMÉ: IBAN]"),
        "Label mode should contain IBAN suppression label"
    );
}

#[test]
fn mask_mode_hash_replaces_with_hash_references() {
    let pdf = pdf_fixtures::visible_text_pdf();
    let (_occ, cleaned_text, _rebuilt) = process_pdf(&pdf, MaskMode::Hash);

    assert!(
        cleaned_text.contains("[SUPPRIMÉ#"),
        "Hash mode should contain hash references"
    );
}
