/// Optional visual inspection integration test.
///
/// When `PDF_CLEANROOM_VISUAL=1` is set, generates all runtime PDF fixtures
/// into `target/pdf-cleanroom-visual/` and renders them to PNG files if an
/// external renderer (`pdftoppm`, `mutool`, or Chromium headless) is available.
///
/// Normal `cargo test` ignores this. Run with:
/// ```sh
/// PDF_CLEANROOM_VISUAL=1 cargo test -- --ignored visual
/// ```
///
/// Generated files (PDF and PNG) are gitignored. No binary fixtures are
/// committed.

use std::path::{Path, PathBuf};
use std::process::Command;

mod support;
use support::pdf_fixtures;

// ---------------------------------------------------------------------------
// Fixture catalogue
// ---------------------------------------------------------------------------

struct Fixture {
    name: &'static str,
    generator: fn() -> Vec<u8>,
}

fn all_fixtures() -> Vec<Fixture> {
    vec![
        Fixture { name: "visible-text",            generator: pdf_fixtures::visible_text_pdf },
        Fixture { name: "metadata-secrets",         generator: pdf_fixtures::metadata_secrets_pdf },
        Fixture { name: "xmp-metadata",             generator: pdf_fixtures::xmp_metadata_pdf },
        Fixture { name: "embedded-file",            generator: pdf_fixtures::embedded_file_pdf },
        Fixture { name: "annotation-secrets",       generator: pdf_fixtures::annotation_secrets_pdf },
        Fixture { name: "form-field-secrets",       generator: pdf_fixtures::form_field_secrets_pdf },
        Fixture { name: "hidden-white-text",        generator: pdf_fixtures::hidden_white_text_pdf },
        Fixture { name: "text-outside-bounds",      generator: pdf_fixtures::text_outside_bounds_pdf },
        Fixture { name: "tiny-text",               generator: pdf_fixtures::tiny_text_pdf },
        Fixture { name: "text-covered-by-shape",    generator: pdf_fixtures::text_covered_by_shape_pdf },
        Fixture { name: "fragmented-secrets",       generator: pdf_fixtures::fragmented_secrets_pdf },
        Fixture { name: "image-only",               generator: pdf_fixtures::image_only_pdf },
        Fixture { name: "image-with-text",          generator: pdf_fixtures::image_with_text_pdf },
        Fixture { name: "image-embedded-only",      generator: pdf_fixtures::image_embedded_only_pdf },
        Fixture { name: "image-embedded-with-text", generator: pdf_fixtures::image_embedded_with_text_pdf },
    ]
}

// ---------------------------------------------------------------------------
// Renderer detection
// ---------------------------------------------------------------------------

fn command_available(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().is_ok()
        || Command::new(cmd).arg("-v").output().is_ok()
}

fn find_renderer() -> Option<Renderer> {
    if command_available("pdftoppm") {
        return Some(Renderer {
            name: "pdftoppm",
            render: Box::new(render_pdftoppm),
        });
    }
    if command_available("mutool") {
        return Some(Renderer {
            name: "mutool",
            render: Box::new(render_mutool),
        });
    }
    // Chromium fallback — tries several binary names
    for bin in &["chromium", "google-chrome", "chromium-browser"] {
        if command_available(bin) {
            let bin_str = *bin;
            return Some(Renderer {
                name: bin_str,
                render: Box::new(move |name, pdf, out| render_chromium(bin_str, name, pdf, out)),
            });
        }
    }
    None
}

struct Renderer {
    name: &'static str,
    render: Box<dyn Fn(&str, &Path, &Path) -> Result<(), String>>,
}

// ---------------------------------------------------------------------------
// Renderer implementations
// ---------------------------------------------------------------------------

/// Render with `pdftoppm` (part of poppler-utils).
///
/// Produces `{prefix}-1.png` (one per page).
fn render_pdftoppm(name: &str, pdf_path: &Path, out_dir: &Path) -> Result<(), String> {
    let prefix = out_dir.join(name);
    let status = Command::new("pdftoppm")
        .arg("-png")
        .arg(pdf_path)
        .arg(&prefix)
        .status()
        .map_err(|e| format!("failed to launch pdftoppm: {e}"))?;
    if !status.success() {
        return Err(format!("pdftoppm exited with {status:?}"));
    }
    let png_path = format!("{}-1.png", prefix.display());
    if !Path::new(&png_path).exists() {
        return Err(format!("pdftoppm produced no output for {name}"));
    }
    Ok(())
}

/// Render with `mutool draw` (part of MuPDF).
///
/// Produces `{name}.png`.
fn render_mutool(name: &str, pdf_path: &Path, out_dir: &Path) -> Result<(), String> {
    let output_path = out_dir.join(format!("{name}.png"));
    let status = Command::new("mutool")
        .arg("draw")
        .arg("-o")
        .arg(&output_path)
        .arg(pdf_path)
        .status()
        .map_err(|e| format!("failed to launch mutool: {e}"))?;
    if !status.success() {
        return Err(format!("mutool exited with {status:?}"));
    }
    if !output_path.exists() {
        return Err(format!("mutool produced no output for {name}"));
    }
    Ok(())
}

/// Render with Chromium headless.
///
/// Launches the browser, opens the PDF as `file://`, and takes a screenshot.
/// This is a fallback when poppler/MuPDF are not installed.
fn render_chromium(bin: &str, name: &str, pdf_path: &Path, out_dir: &Path) -> Result<(), String> {
    let output_path = out_dir.join(format!("{name}.png"));
    let url = format!("file://{}", pdf_path.canonicalize().map_err(|e| format!("canonicalize: {e}"))?.display());
    let status = Command::new(bin)
        .arg("--headless")
        .arg("--disable-gpu")
        .arg("--no-sandbox")
        .arg(format!("--screenshot={}", output_path.display()))
        .arg("--window-size=800,600")
        .arg(&url)
        .status()
        .map_err(|e| format!("failed to launch {bin}: {e}"))?;
    if !status.success() {
        return Err(format!("{bin} exited with {status:?}"));
    }
    if !output_path.exists() {
        return Err(format!("{bin} produced no output for {name}"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn visual_inspection_of_fixtures() {
    // Gate: only run when env var is set
    let env_val = std::env::var("PDF_CLEANROOM_VISUAL").unwrap_or_default();
    if env_val != "1" {
        eprintln!(
            concat!(
                "skipping visual inspection; ",
                "set PDF_CLEANROOM_VISUAL=1 to generate PNG outputs in ",
                "target/pdf-cleanroom-visual/"
            )
        );
        return;
    }

    let out_dir = PathBuf::from("target/pdf-cleanroom-visual");
    std::fs::create_dir_all(&out_dir)
        .expect("failed to create target/pdf-cleanroom-visual/");

    let fixtures = all_fixtures();
    let mut pdf_count = 0u32;

    // Phase 1: generate all PDFs
    for f in &fixtures {
        let bytes = (f.generator)();
        let pdf_path = out_dir.join(format!("{}.pdf", f.name));
        std::fs::write(&pdf_path, &bytes).unwrap_or_else(|e| {
            panic!("failed to write {}: {e}", pdf_path.display())
        });
        pdf_count += 1;
    }

    eprintln!(
        "generated {pdf_count} PDF(s) in {}",
        out_dir.canonicalize().unwrap_or(out_dir.clone()).display()
    );

    // Phase 2: render to PNG
    let renderer = find_renderer();

    let renderer = match renderer {
        Some(r) => r,
        None => {
            eprintln!(
                "no PDF renderer found (tried pdftoppm, mutool, chromium).\n\
                 Generated PDFs are available at: {}",
                out_dir.canonicalize().unwrap_or(out_dir.clone()).display(),
            );
            return;
        }
    };

    eprintln!("rendering with {}...", renderer.name);

    let mut ok = 0u32;
    let mut err = 0u32;

    for f in &fixtures {
        let pdf_path = out_dir.join(format!("{}.pdf", f.name));
        match (renderer.render)(f.name, &pdf_path, &out_dir) {
            Ok(()) => {
                ok += 1;
            }
            Err(msg) => {
                eprintln!("  [{name}] WARN: {msg}", name = f.name);
                err += 1;
            }
        }
    }

    eprintln!(
        "rendered {ok}/{total} PDF(s) to PNG with {}",
        renderer.name,
        total = fixtures.len(),
    );
    if err > 0 {
        eprintln!("{err} rendering failure(s) — check individual warnings above");
    }
}
