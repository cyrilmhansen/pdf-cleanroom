//! pdf-cleanroom — PDF document sanitization tool.
//!
//! Scans PDF documents for secrets (emails, phone numbers, IBANs)
//! and rebuilds a clean PDF with those secrets masked.
mod cli;

use clap::Parser;
use cli::{Cli, Command, Strategy};



use pdf_cleanroom::{
    detect::Detector,
    mask::{self, MaskMode},
    pdf_extract, rebuild, report, safety,
};

fn main() {
    let cli = Cli::parse();

    let mask_mode = MaskMode::from_str(&cli.mask).unwrap_or(MaskMode::BlackBlock);

    let result = match &cli.command {
        Command::Scan { input, report } => cmd_scan(input, report.as_deref(), &cli),
        Command::Rebuild {
            input,
            output,
            report,
        } => cmd_rebuild(input, output, report.as_deref(), mask_mode, &cli),
        Command::Preserve { .. } => {
            safety::check_preserve_not_implemented().map_err(|e| {
                eprintln!("ERROR: {e}");
                std::process::exit(1);
            }).ok();
            unreachable!()
        }
    };

    if let Err(e) = result {
        eprintln!("ERROR: {e}");
        std::process::exit(1);
    }
}

/// Execute the `scan` command.
fn cmd_scan(
    input: &str,
    report_path: Option<&str>,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = pdf_extract::extract(input)?;
    let detector = Detector::new();
    let mut rep = report::Report::new(content.num_pages, cli.unsafe_show_secrets);

    for page in &content.pages {
        let detections = detector.scan_text(&page.text);
        for d in detections {
            rep.add_occurrence(
                page.page_num,
                &d.kind,
                if cli.unsafe_show_secrets {
                    Some(d.value.clone())
                } else {
                    None
                },
                Some(d.start),
                Some(d.end),
                None, // lopdf extract_text doesn't provide coordinates
                mask::apply_mask(&d.value, &d.kind, MaskMode::Label),
            );
        }
    }

    let json = rep.to_json()?;
    println!("{json}");

    if let Some(path) = report_path {
        std::fs::write(path, &json)?;
        eprintln!("Report written to {path}");
    }

    eprintln!(
        "Scan complete: {} page(s), {} secret(s) detected.",
        content.num_pages, rep.total_secrets
    );
    Ok(())
}

/// Execute the `rebuild` command.
fn cmd_rebuild(
    input: &str,
    output: &str,
    report_path: Option<&str>,
    mask_mode: MaskMode,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    // Early exit for flatten-raster: no text extraction, no detection, no masking.
    if cli.strategy == Strategy::FlattenRaster {
        let renderer = pdf_cleanroom::flatten::detect_renderer()
            .ok_or_else(|| {
                let msg = "no PDF renderer found — install poppler-utils (pdftoppm), \
                           mupdf-tools (mutool), or ghostscript (gs)";
                eprintln!("ERROR: --strategy flatten-raster requires an external PDF renderer.\n{msg}");
                Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, msg))
                    as Box<dyn std::error::Error>
            })?;
        pdf_cleanroom::flatten::flatten_pdf(input, output, &renderer)?;
        eprintln!(
            "flatten-raster complete: rendered {} page(s) to image-only PDF at {output}",
            pdf_extract::extract(input)?.num_pages,
        );
        eprintln!(
            "WARNING: Output is an image-only PDF. Text is not selectable. \
             Visible secrets in the rendered image are NOT masked."
        );
        return Ok(());
    }

    // 1. Extract text from source PDF
    let content = pdf_extract::extract(input)?;

    // 2. Detect secrets
    let detector = Detector::new();
    let mut rep = report::Report::new(content.num_pages, cli.unsafe_show_secrets);
    let mut clean_pages = Vec::new();

    for page in &content.pages {
        let detections = detector.scan_text(&page.text);

        // Add to report
        for d in &detections {
            rep.add_occurrence(
                page.page_num,
                &d.kind,
                if cli.unsafe_show_secrets {
                    Some(d.value.clone())
                } else {
                    None
                },
                Some(d.start),
                Some(d.end),
                None,
                mask::apply_mask(&d.value, &d.kind, MaskMode::Label),
            );
        }

        // Apply masks to text
        let cleaned_text: String = rebuild::apply_masks_to_text(&page.text, &detections, mask_mode).0;


        // Split into lines for rendering
        let lines: Vec<String> = cleaned_text
            .lines()
            .map(|l| l.to_string())
            .collect();

        clean_pages.push(rebuild::CleanPage {
            page_num: page.page_num,
            lines,
        });
    }

    // Write report if requested (even with --dry-run)
    if let Some(path) = report_path {
        let json = rep.to_json()?;
        std::fs::write(path, &json)?;
        eprintln!("Report written to {path}");
    }

    eprintln!(
        "Analysis complete: {} page(s), {} secret(s) detected.",
        content.num_pages, rep.total_secrets
    );

    // 3. Rebuild PDF (unless --dry-run)
    if cli.dry_run {
        eprintln!("--dry-run: PDF not modified.");
        eprintln!("Output would be: {output}");
        return Ok(());
    }

    // Warn about limitations when flatten-visible is selected
    if cli.strategy == Strategy::FlattenVisible {
        eprintln!(
            "WARNING: --strategy flatten-visible is experimental. \
             Image sanitization is not yet implemented. \
             Images from the source PDF are not preserved in the output."
        );
    }


    let metadata = rebuild::RebuildMetadata {
        title: format!("pdf-cleanroom — {}", input),
    };
    rebuild::rebuild(output, &clean_pages, &metadata)?;
    eprintln!("Clean PDF written to {output}");

    Ok(())
}
