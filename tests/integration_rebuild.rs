/// Integration test: rebuild a PDF and verify secrets are removed.
mod support;

use support::pdf_fixtures;

use std::fs;
use std::io::BufWriter;
use std::path::PathBuf;

use printpdf::*;

use pdf_cleanroom::detect::Detector;
use pdf_cleanroom::mask::MaskMode;
use pdf_cleanroom::{pdf_extract, rebuild};

/// Helper: generate a tiny PDF with secrets for rebuild testing.
fn generate_test_pdf(path: &PathBuf) {
    let (doc, page, layer) = PdfDocument::new(
        "rebuild-test",
        Mm(100.0),
        Mm(100.0),
        "Content",
    );
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .expect("font");
    let layer = doc.get_page(page).get_layer(layer);

    layer.use_text(
        "Email: user@test.com, Tel: 01 23 45 67 89, IBAN: FR7630006000011234567890189",
        8.0,
        Mm(10.0),
        Mm(80.0),
        &font,
    );

    doc.save(&mut BufWriter::new(fs::File::create(path).expect("create pdf")))
        .expect("save pdf");
}

#[test]
fn test_rebuild_removes_secrets() {
    let dir = std::env::temp_dir().join("pdf_cleanroom_test");
    fs::create_dir_all(&dir).ok();
    let input_path = dir.join("test_rebuild_input.pdf");
    let output_path = dir.join("test_rebuild_output.pdf");

    generate_test_pdf(&input_path);

    let content = pdf_extract::extract(
        input_path.to_str().expect("input path"),
    )
    .expect("extract");

    let detector = Detector::new();
    let mut clean_pages = Vec::new();

    for page in &content.pages {
        let detections = detector.scan_text(&page.text);
        let cleaned_text: String =
            rebuild::apply_masks_to_text(&page.text, &detections, MaskMode::BlackBlock).0;

        let lines: Vec<String> = cleaned_text.lines().map(|l| l.to_string()).collect();
        clean_pages.push(rebuild::CleanPage {
            page_num: page.page_num,
            lines,
        });
    }

    let metadata = rebuild::RebuildMetadata {
        title: "rebuild-test".into(),
    };
    rebuild::rebuild(
        output_path.to_str().expect("output path"),
        &clean_pages,
        &metadata,
    )
    .expect("rebuild");

    assert!(output_path.exists(), "output PDF should exist");
    assert!(
        output_path.metadata().unwrap().len() > 100,
        "output PDF should have non-trivial size"
    );

    // Extract text from the rebuilt PDF and verify no secrets remain
    let rebuilt_content = pdf_extract::extract(
        output_path.to_str().expect("output path"),
    )
    .expect("extract rebuilt");

    let rebuilt_text: String = rebuilt_content
        .pages
        .iter()
        .map(|p| p.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !rebuilt_text.contains("user@test.com"),
        "email should not appear in rebuilt PDF"
    );
    assert!(
        !rebuilt_text.contains("01 23 45 67 89"),
        "phone should not appear in rebuilt PDF"
    );
    assert!(
        !rebuilt_text.contains("FR7630006000011234567890189"),
        "IBAN should not appear in rebuilt PDF"
    );

    assert!(
        rebuilt_text.contains("Email"),
        "text labels should remain in rebuilt PDF"
    );

    // Scan the rebuilt PDF and confirm no secrets are detected
    let rebuilt_results = detector.scan_text(&rebuilt_text);
    assert!(
        rebuilt_results.is_empty(),
        "no secrets should be detected in rebuilt PDF, got {:?}",
        rebuilt_results.iter().map(|r| &r.kind).collect::<Vec<_>>()
    );

    fs::remove_file(&input_path).ok();
    fs::remove_file(&output_path).ok();
    fs::remove_dir(&dir).ok();
}

#[test]
fn test_rebuild_empty_text_creates_valid_pdf() {
    let dir = std::env::temp_dir().join("pdf_cleanroom_test");
    fs::create_dir_all(&dir).ok();
    let output_path = dir.join("test_rebuild_empty.pdf");

    let clean_pages = vec![rebuild::CleanPage {
        page_num: 1,
        lines: vec!["Aucun secret détecté.".to_string()],
    }];

    let metadata = rebuild::RebuildMetadata {
        title: "clean-doc".into(),
    };
    rebuild::rebuild(
        output_path.to_str().expect("path"),
        &clean_pages,
        &metadata,
    )
    .expect("rebuild empty");

    assert!(output_path.exists());
    assert!(
        output_path.metadata().unwrap().len() > 100,
        "output PDF should have non-trivial size"
    );

    let content =
        pdf_extract::extract(output_path.to_str().expect("path")).expect("extract");
    assert!(content.num_pages >= 1);

    fs::remove_file(&output_path).ok();
    fs::remove_dir(&dir).ok();
}

// ---------------------------------------------------------------------------
// Rebuild quality tests
// ---------------------------------------------------------------------------

#[test]
fn test_rebuild_preserves_accented_text() {
    let pdf_bytes = pdf_fixtures::accented_text_pdf();
    let content =
        pdf_extract::extract_bytes(&pdf_bytes).expect("extract accented PDF");

    // Verify extraction preserves accented chars before rebuild
    let all_text: String = content.pages.iter().map(|p| p.text.clone()).collect();
    assert!(all_text.contains("é"), "extracted text should contain é");
    assert!(all_text.contains("è"), "extracted text should contain è");
    assert!(all_text.contains("ê"), "extracted text should contain ê");
    assert!(all_text.contains("ç"), "extracted text should contain ç");
    assert!(all_text.contains("€"), "extracted text should contain €");

    // Rebuild
    let lines: Vec<String> = all_text.lines().map(|l| l.to_string()).collect();
    let clean_pages = vec![rebuild::CleanPage {
        page_num: 1,
        lines,
    }];
    let metadata = rebuild::RebuildMetadata {
        title: "accented-test".into(),
    };
    let output_path = std::env::temp_dir()
        .join(format!("pdf_cleanroom_rebuild_accented_{}.pdf", std::process::id()));
    rebuild::rebuild(
        output_path.to_str().expect("path"),
        &clean_pages,
        &metadata,
    )
    .expect("rebuild accented");

    // Extract from rebuilt PDF
    let rebuilt_content =
        pdf_extract::extract(output_path.to_str().expect("path")).expect("extract rebuilt");
    let rebuilt_text: String = rebuilt_content
        .pages
        .iter()
        .map(|p| p.text.clone())
        .collect();

    // Accented characters must survive rebuild
    assert!(
        rebuilt_text.contains("Français"),
        "label 'Français' should survive rebuild, got: {rebuilt_text:?}"
    );
    assert!(rebuilt_text.contains("€"), "€ should survive rebuild");
    assert!(
        rebuilt_text.contains("1 250"),
        "numbers with formatting should survive"
    );

    std::fs::remove_file(&output_path).ok();
}

#[test]
fn test_rebuild_removes_secrets_from_admin_document() {
    let pdf_bytes = pdf_fixtures::admin_document_pdf();
    let content =
        pdf_extract::extract_bytes(&pdf_bytes).expect("extract admin PDF");
    let all_text: String = content.pages.iter().map(|p| p.text.clone()).collect();

    // Verify secrets are detected
    let detector = pdf_cleanroom::detect::Detector::new();
    let detections = detector.scan_text(&all_text);
    let kinds: Vec<&str> = detections.iter().map(|d| d.kind.as_str()).collect();
    assert!(
        kinds.contains(&"iban"),
        "should detect IBAN in admin doc, got: {kinds:?}"
    );
    assert!(
        kinds.contains(&"phone_fr"),
        "should detect phone in admin doc, got: {kinds:?}"
    );
    assert!(
        kinds.contains(&"email"),
        "should detect email in admin doc, got: {kinds:?}"
    );

    // Rebuild with masks
    let (cleaned_text, _replacements) =
        rebuild::apply_masks_to_text(&all_text, &detections, pdf_cleanroom::mask::MaskMode::Label);
    let lines: Vec<String> = cleaned_text.lines().map(|l| l.to_string()).collect();

    // Check labels survived but secrets are masked
    let combined = lines.join("\n");
    assert!(combined.contains("RELEVÉ DE COMPTE"), "heading should survive");
    assert!(combined.contains("Titulaire:"), "label should survive");
    assert!(combined.contains("Virement entrant"), "transaction label should survive");
    assert!(combined.contains("Solde après opération"), "balance label should survive");

    // Secrets must NOT appear
    assert!(
        !combined.contains("FR76 3000 6000 0112 3456 7890 189"),
        "IBAN should be masked"
    );
    assert!(!combined.contains("06 11 22 33 44"), "phone should be masked");
    assert!(
        !combined.contains("jean.demo@example.com"),
        "email should be masked"
    );

    // Rebuild to PDF and verify extraction
    let clean_pages = vec![rebuild::CleanPage {
        page_num: 1,
        lines,
    }];
    let metadata = rebuild::RebuildMetadata {
        title: "admin-test".into(),
    };
    let output_path = std::env::temp_dir()
        .join(format!("pdf_cleanroom_rebuild_admin_{}.pdf", std::process::id()));
    rebuild::rebuild(
        output_path.to_str().expect("path"),
        &clean_pages,
        &metadata,
    )
    .expect("rebuild admin");

    // Extract rebuilt PDF
    let rebuilt_content =
        pdf_extract::extract(output_path.to_str().expect("path")).expect("extract rebuilt");
    let rebuilt_text: String = rebuilt_content
        .pages
        .iter()
        .map(|p| p.text.clone())
        .collect();

    // Secrets must be absent from rebuilt PDF text
    assert!(
        !rebuilt_text.contains("FR76 3000 6000 0112 3456 7890 189"),
        "IBAN must not appear in rebuilt text"
    );
    assert!(
        !rebuilt_text.contains("06 11 22 33 44"),
        "phone must not appear in rebuilt text"
    );
    assert!(
        !rebuilt_text.contains("jean.demo@example.com"),
        "email must not appear in rebuilt text"
    );

    std::fs::remove_file(&output_path).ok();
}

#[test]
fn test_rebuild_text_order_is_readable() {
    let pdf_bytes = pdf_fixtures::admin_document_pdf();
    let content =
        pdf_extract::extract_bytes(&pdf_bytes).expect("extract admin PDF");
    let all_text: String = content.pages.iter().map(|p| p.text.clone()).collect();

    // Apply masks
    let detector = pdf_cleanroom::detect::Detector::new();
    let detections = detector.scan_text(&all_text);
    let (cleaned_text, _replacements) =
        rebuild::apply_masks_to_text(&all_text, &detections, pdf_cleanroom::mask::MaskMode::Label);
    let lines: Vec<String> = cleaned_text.lines().map(|l| l.to_string()).collect();

    // Rebuild
    let clean_pages = vec![rebuild::CleanPage {
        page_num: 1,
        lines,
    }];
    let metadata = rebuild::RebuildMetadata {
        title: "order-test".into(),
    };
    let output_path = std::env::temp_dir()
        .join(format!("pdf_cleanroom_rebuild_order_{}.pdf", std::process::id()));
    rebuild::rebuild(
        output_path.to_str().expect("path"),
        &clean_pages,
        &metadata,
    )
    .expect("rebuild order");

    // Extract rebuilt PDF
    let rebuilt_content =
        pdf_extract::extract(output_path.to_str().expect("path")).expect("extract rebuilt");
    let rebuilt_text: String = rebuilt_content
        .pages
        .iter()
        .map(|p| p.text.clone())
        .collect();

    // Key labels must appear in readable order (heading before details)
    let heading_pos = rebuilt_text.find("RELEVÉ DE COMPTE");
    let titulaire_pos = rebuilt_text.find("Titulaire:");
    let virement_pos = rebuilt_text.find("Virement entrant");

    assert!(heading_pos.is_some(), "heading must be present");
    assert!(titulaire_pos.is_some(), "titulaire label must be present");
    assert!(virement_pos.is_some(), "virement label must be present");

    // Heading should appear before details
    assert!(
        heading_pos.unwrap() < titulaire_pos.unwrap(),
        "heading should appear before titulaire"
    );
    // Titulaire should appear before transaction
    assert!(
        titulaire_pos.unwrap() < virement_pos.unwrap(),
        "titulaire should appear before virement"
    );

    std::fs::remove_file(&output_path).ok();
}
