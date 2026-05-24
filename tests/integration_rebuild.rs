/// Integration test: rebuild a PDF and verify secrets are removed.

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
