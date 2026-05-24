/// Integration test: scan a PDF and verify secret detection.
///
/// These tests generate a small PDF with known secrets using printpdf,
/// then verify that the scan module detects them correctly.

use std::fs;
use std::io::BufWriter;
use std::path::PathBuf;

use printpdf::*;

use pdf_cleanroom::detect::Detector;
use pdf_cleanroom::mask::{self, MaskMode};
use pdf_cleanroom::{pdf_extract, report};

/// Helper: generate a tiny PDF with embedded secrets for testing.
fn generate_test_pdf(path: &PathBuf) {
    let (doc, page, layer) = PdfDocument::new(
        "scan-test",
        Mm(100.0),
        Mm(100.0),
        "Content",
    );
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .expect("font");
    let layer = doc.get_page(page).get_layer(layer);

    layer.use_text(
        "Contact: jean.dupont@example.com",
        10.0,
        Mm(10.0),
        Mm(80.0),
        &font,
    );

    layer.use_text(
        "Tél: 06 11 22 33 44",
        10.0,
        Mm(10.0),
        Mm(70.0),
        &font,
    );

    layer.use_text(
        "IBAN: FR76 3000 6000 0112 3456 7890 189",
        10.0,
        Mm(10.0),
        Mm(60.0),
        &font,
    );

    layer.use_text(
        "Aucun secret ici.",
        10.0,
        Mm(10.0),
        Mm(50.0),
        &font,
    );

    doc.save(&mut BufWriter::new(fs::File::create(path).expect("create pdf")))
        .expect("save pdf");
}

#[test]
fn test_scan_detects_secrets_in_pdf() {
    let dir = std::env::temp_dir().join("pdf_cleanroom_test");
    fs::create_dir_all(&dir).ok();
    let pdf_path = dir.join("test_scan.pdf");

    generate_test_pdf(&pdf_path);

    let content = pdf_extract::extract(
        pdf_path.to_str().expect("path"),
    )
    .expect("extract text");

    assert!(
        content.num_pages >= 1,
        "should have at least one page"
    );

    let detector = Detector::new();
    let all_text: String = content
        .pages
        .iter()
        .map(|p| p.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let results = detector.scan_text(&all_text);

    let kinds: Vec<&str> = results.iter().map(|r| r.kind.as_str()).collect();
    assert!(
        kinds.contains(&"email"),
        "should detect email, got: {:?}",
        kinds
    );
    assert!(
        kinds.contains(&"phone_fr"),
        "should detect phone_fr, got: {:?}",
        kinds
    );
    assert!(
        kinds.contains(&"iban"),
        "should detect IBAN, got: {:?}",
        kinds
    );

    fs::remove_file(&pdf_path).ok();
    fs::remove_dir(&dir).ok();
}

#[test]
fn test_scan_report_contains_masked_secrets() {
    let dir = std::env::temp_dir().join("pdf_cleanroom_test");
    fs::create_dir_all(&dir).ok();
    let pdf_path = dir.join("test_scan_report.pdf");

    generate_test_pdf(&pdf_path);

    let content =
        pdf_extract::extract(pdf_path.to_str().expect("path")).expect("extract");
    let detector = Detector::new();
    let mut rep = report::Report::new(content.num_pages, false);

    for page in &content.pages {
        let results = detector.scan_text(&page.text);
        for d in results {
            let masked = mask::apply_mask(&d.value, &d.kind, MaskMode::Label);
            rep.add_occurrence(
                page.page_num,
                &d.kind,
                None,
                Some(d.start),
                Some(d.end),
                None,
                masked,
            );
        }
    }

    assert!(rep.total_secrets >= 3, "should detect at least 3 secrets");
    assert!(
        rep.by_kind.contains_key("email"),
        "should have email in by_kind"
    );
    assert!(
        rep.by_kind.contains_key("phone_fr"),
        "should have phone_fr in by_kind"
    );
    assert!(
        rep.by_kind.contains_key("iban"),
        "should have iban in by_kind"
    );

    for occ in &rep.occurrences {
        assert!(
            !occ.masked_display.contains("jean.dupont@example.com"),
            "exact email should be masked in report"
        );
        assert!(
            !occ.masked_display.contains("06 11 22 33 44"),
            "exact phone should be masked in report"
        );
    }

    for occ in &rep.occurrences {
        assert_eq!(occ.sha256.len(), 64, "sha256 should be 64 hex chars");
    }

    let json = rep.to_json().expect("serialize");
    assert!(
        json.contains("email"),
        "JSON report should contain kind email"
    );
    assert!(
        json.contains("by_kind"),
        "JSON report should contain by_kind"
    );

    fs::remove_file(&pdf_path).ok();
    fs::remove_dir(&dir).ok();
}
