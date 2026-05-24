/// Flatten-raster integration test — optional, gated by `PDF_CLEANROOM_FLATTEN_TESTS=1`.
///
/// # Prerequisites
///
/// Requires an external PDF renderer on PATH:
/// - `pdftoppm` (poppler-utils)
/// - or `mutool` (mupdf-tools)
/// - or `gs` (ghostscript)
///
/// If no renderer is found, the test is skipped with a clear message.
///
/// # Running
///
/// ```sh
/// PDF_CLEANROOM_FLATTEN_TESTS=1 cargo test --test integration_flatten
/// ```
///
/// Normal `cargo test` never depends on external tools.
use std::path::PathBuf;
use std::io::Read;
use std::process::Command;

mod support;
use support::pdf_fixtures;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check whether a CLI tool is available on PATH.
fn tool_available(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().is_ok()
        || Command::new(cmd).arg("-v").output().is_ok()
}

/// Detect the first available PDF renderer.
fn find_renderer() -> Option<String> {
    for name in &["pdftoppm", "mutool", "gs"] {
        if tool_available(name) {
            return Some(name.to_string());
        }
    }
    None
}

/// Assert that a path is a valid PDF (loadable by lopdf) and has non-zero size.
fn assert_valid_pdf(path: &std::path::Path) {
    let metadata =
        std::fs::metadata(path).unwrap_or_else(|e| panic!("failed to stat {path:?}: {e}"));
    assert!(
        metadata.len() > 100,
        "PDF too small: {} bytes",
        metadata.len()
    );

    // Verify it loads with lopdf
    let result = lopdf::Document::load(path);
    assert!(
        result.is_ok(),
        "output PDF should be loadable by lopdf but got: {:?}",
        result.err(),
    );
}

/// Assert that normal text extraction finds no text in a PDF byte slice.
fn assert_no_extractable_text(pdf_bytes: &[u8], label: &str) {
    use pdf_cleanroom::pdf_extract::extract_bytes;
    match extract_bytes(pdf_bytes) {
        Ok(content) => {
            let text: String = content.pages.into_iter().map(|p| p.text).collect();
            assert!(
                text.trim().is_empty(),
                "[{label}] normal text extraction should return no text for image-only PDF, \
                 but extracted: {text:?}",
            );
        }
        Err(e) => {
            // Image-only PDF may fail extraction entirely (lopdf may return
            // an error for pages with no text content)—that's expected.
            eprintln!("[{label}] normal text extraction failed (expected for image-only PDF): {e}");
        }
    }
}

/// Assert that obvious source secret bytes were not copied into the output PDF.
fn assert_no_source_secret_bytes(pdf_bytes: &[u8]) {
    for secret in [
        pdf_fixtures::SECRET_EMAIL,
        pdf_fixtures::SECRET_PHONE,
        pdf_fixtures::SECRET_PHONE_COMPACT,
        pdf_fixtures::SECRET_IBAN,
    ] {
        assert!(
            !pdf_bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()),
            "flattened PDF should not contain source secret bytes: {secret}",
        );
    }
}

fn assert_pdftotext_finds_no_secret(path: &std::path::Path) {
    if !tool_available("pdftotext") {
        eprintln!("pdftotext not found; skipping external text extraction check");
        return;
    }

    let output = Command::new("pdftotext")
        .arg(path)
        .arg("-")
        .output()
        .unwrap_or_else(|e| panic!("failed to run pdftotext: {e}"));
    assert!(
        output.status.success(),
        "pdftotext should succeed for flattened output: {:?}",
        output.status,
    );

    let text = String::from_utf8_lossy(&output.stdout);
    for secret in [
        pdf_fixtures::SECRET_EMAIL,
        pdf_fixtures::SECRET_PHONE,
        pdf_fixtures::SECRET_PHONE_COMPACT,
        pdf_fixtures::SECRET_IBAN,
    ] {
        assert!(
            !text.contains(secret),
            "pdftotext output should not contain secret {secret}: {text:?}",
        );
    }
}

fn assert_image_only_structure(path: &std::path::Path) {
    let doc = lopdf::Document::load(path).expect("flattened PDF should load with lopdf");

    let has_font_object = doc.objects.iter().any(|(_, obj)| {
        obj.as_dict()
            .ok()
            .and_then(|d| d.get(b"Type").ok())
            .and_then(|t| t.as_name_str().ok())
            .is_some_and(|name| name == "Font")
    });
    assert!(
        !has_font_object,
        "flattened PDF should have no /Font objects (image-only PDF)"
    );

    let has_image_object = doc.objects.iter().any(|(_, obj)| {
        obj.as_stream()
            .ok()
            .and_then(|s| s.dict.get(b"Subtype").ok())
            .and_then(|t| t.as_name_str().ok())
            .is_some_and(|name| name == "Image")
    });
    assert!(
        has_image_object,
        "flattened PDF should contain at least one /Image XObject"
    );
}

fn first_decoded_image(path: &std::path::Path) -> (u32, u32, Vec<u8>) {
    let doc = lopdf::Document::load(path).expect("flattened PDF should load with lopdf");
    for (_, obj) in &doc.objects {
        let Ok(stream) = obj.as_stream() else {
            continue;
        };
        let is_image = stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|t| t.as_name_str().ok())
            .is_some_and(|name| name == "Image");
        if !is_image {
            continue;
        }

        let width = stream
            .dict
            .get(b"Width")
            .and_then(|obj| obj.as_i64())
            .expect("image width") as u32;
        let height = stream
            .dict
            .get(b"Height")
            .and_then(|obj| obj.as_i64())
            .expect("image height") as u32;

        let mut decoder = flate2::read::ZlibDecoder::new(stream.content.as_slice());
        let mut decoded = Vec::new();
        decoder
            .read_to_end(&mut decoded)
            .expect("image stream should FlateDecode");
        return (width, height, decoded);
    }

    panic!("flattened PDF should contain an image XObject");
}

fn rgb_at(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 3] {
    let offset = (y as usize * width as usize + x as usize) * 3;
    [pixels[offset], pixels[offset + 1], pixels[offset + 2]]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn flatten_visible_text_pdf() {
    let env_val = std::env::var("PDF_CLEANROOM_FLATTEN_TESTS").unwrap_or_default();
    if env_val != "1" {
        eprintln!("skipping flatten test: PDF_CLEANROOM_FLATTEN_TESTS not set to 1");
        return;
    }

    let renderer = match find_renderer() {
        Some(r) => r,
        None => {
            eprintln!("SKIP: no PDF renderer found (tried pdftoppm, mutool, gs)");
            return;
        }
    };

    let out_dir = PathBuf::from("target/pdf-cleanroom-flatten-test");
    std::fs::create_dir_all(&out_dir).expect("failed to create target/pdf-cleanroom-flatten-test/");

    // Generate source PDF with known secrets
    let src_pdf = out_dir.join("source.pdf");
    let pdf_bytes = pdf_fixtures::visible_text_pdf();
    std::fs::write(&src_pdf, &pdf_bytes)
        .unwrap_or_else(|e| panic!("failed to write source PDF: {e}"));

    let output_pdf = out_dir.join("flattened.pdf");

    // Run flatten-raster
    let result = pdf_cleanroom::flatten::flatten_pdf(
        src_pdf.to_str().unwrap(),
        output_pdf.to_str().unwrap(),
        &renderer,
    );
    assert!(
        result.is_ok(),
        "flatten_pdf failed with renderer={renderer}: {:?}",
        result.err(),
    );

    // Verify output
    assert!(output_pdf.exists(), "output PDF should exist");
    assert_valid_pdf(&output_pdf);

    let flattened_bytes =
        std::fs::read(&output_pdf).unwrap_or_else(|e| panic!("failed to read flattened PDF: {e}"));

    eprintln!(
        "flatten-raster test: {} → {} ({:.1} KB → {:.1} KB, renderer={})",
        src_pdf.display(),
        output_pdf.display(),
        pdf_bytes.len() as f64 / 1024.0,
        flattened_bytes.len() as f64 / 1024.0,
        renderer,
    );

    // Normal text extraction must return no text (image-only PDF)
    assert_no_extractable_text(&flattened_bytes, "flattened");

    // Obvious source secret bytes must not be copied into the rebuilt PDF.
    assert_no_source_secret_bytes(&flattened_bytes);

    assert_image_only_structure(&output_pdf);

    eprintln!(
        "  structural checks passed: no extractable text, no source secret bytes, no /Font objects, /Image XObject present."
    );

    // Clean up
    std::fs::remove_dir_all(&out_dir).ok();
}

#[test]
fn flatten_with_manual_pixel_mask_modifies_embedded_image() {
    let env_val = std::env::var("PDF_CLEANROOM_FLATTEN_TESTS").unwrap_or_default();
    if env_val != "1" {
        eprintln!("skipping pixel mask test: PDF_CLEANROOM_FLATTEN_TESTS not set to 1");
        return;
    }

    let renderer = match find_renderer() {
        Some(r) => r,
        None => {
            eprintln!("SKIP: no PDF renderer found (tried pdftoppm, mutool, gs)");
            return;
        }
    };

    let out_dir = PathBuf::from("target/pdf-cleanroom-pixel-mask-test");
    std::fs::create_dir_all(&out_dir)
        .expect("failed to create target/pdf-cleanroom-pixel-mask-test/");

    let src_pdf = out_dir.join("source.pdf");
    let pdf_bytes = pdf_fixtures::visible_text_pdf();
    std::fs::write(&src_pdf, &pdf_bytes)
        .unwrap_or_else(|e| panic!("failed to write source PDF: {e}"));

    let output_pdf = out_dir.join("masked.pdf");
    let masks = [pdf_cleanroom::flatten::MaskRegion {
        page: 1,
        x: 0,
        y: 0,
        width: 32,
        height: 32,
    }];

    let result = pdf_cleanroom::flatten::flatten_pdf_with_masks(
        src_pdf.to_str().unwrap(),
        output_pdf.to_str().unwrap(),
        &renderer,
        &masks,
    );
    assert!(
        result.is_ok(),
        "flatten_pdf_with_masks failed with renderer={renderer}: {:?}",
        result.err(),
    );

    assert!(output_pdf.exists(), "masked output PDF should exist");
    assert_valid_pdf(&output_pdf);

    let masked_bytes =
        std::fs::read(&output_pdf).unwrap_or_else(|e| panic!("failed to read masked PDF: {e}"));
    assert_no_extractable_text(&masked_bytes, "masked");
    assert_pdftotext_finds_no_secret(&output_pdf);
    assert_no_source_secret_bytes(&masked_bytes);
    assert_image_only_structure(&output_pdf);

    let (width, height, pixels) = first_decoded_image(&output_pdf);
    assert!(
        width >= 64 && height >= 64,
        "rendered image should be non-trivial"
    );
    assert_eq!(rgb_at(&pixels, width, 0, 0), [0, 0, 0]);
    assert_eq!(rgb_at(&pixels, width, 31, 31), [0, 0, 0]);
    assert_ne!(
        rgb_at(&pixels, width, 40, 40),
        [0, 0, 0],
        "pixel outside manual mask should not be unexpectedly blacked out",
    );

    std::fs::remove_dir_all(&out_dir).ok();
}
