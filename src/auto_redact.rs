//! Automatic pixel-mask derivation from selectable PDF text coordinates.
//!
//! This module intentionally uses only Poppler `pdftotext -bbox` output. It
//! does not OCR images and does not inspect or copy source PDF objects.

use std::process::Command;

use regex::Regex;

use crate::detect::Detector;
use crate::flatten::MaskRegion;
use crate::mask::{self, MaskMode};
use crate::report::{Coords, Report};

/// Summary of automatic mask derivation.
#[derive(Debug, Clone)]
pub struct AutoMaskResult {
    pub masks: Vec<MaskRegion>,
    pub report: Report,
    pub unmapped_findings: usize,
}

/// Derive page-space mask regions from `pdftotext -bbox` word coordinates.
pub fn detect_text_masks(
    input_path: &str,
    unsafe_show_secrets: bool,
) -> Result<AutoMaskResult, Box<dyn std::error::Error>> {
    let output = Command::new("pdftotext")
        .arg("-bbox")
        .arg(input_path)
        .arg("-")
        .output()
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("automatic pixel masking requires pdftotext -bbox from Poppler: {e}"),
            )
        })?;

    if !output.status.success() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "automatic pixel masking requires pdftotext -bbox from Poppler",
        )));
    }

    let xml = String::from_utf8_lossy(&output.stdout);
    let pages = parse_bbox_words(&xml)?;
    Ok(build_masks_from_pages(&pages, unsafe_show_secrets))
}

#[derive(Debug, Clone)]
struct BboxPage {
    page_num: usize,
    words: Vec<PositionedWord>,
}

#[derive(Debug, Clone)]
struct PositionedWord {
    text: String,
    x_min: f32,
    y_min: f32,
    x_max: f32,
    y_max: f32,
}

fn parse_bbox_words(xml: &str) -> Result<Vec<BboxPage>, Box<dyn std::error::Error>> {
    let page_re = Regex::new(r#"<page\b[^>]*width="([0-9.]+)"[^>]*height="([0-9.]+)"[^>]*>"#)?;
    let word_re = Regex::new(
        r#"<word\b[^>]*xMin="([0-9.]+)"[^>]*yMin="([0-9.]+)"[^>]*xMax="([0-9.]+)"[^>]*yMax="([0-9.]+)"[^>]*>(.*?)</word>"#,
    )?;

    let mut pages = Vec::new();
    let mut current: Option<BboxPage> = None;

    for line in xml.lines() {
        if page_re.is_match(line) {
            if let Some(page) = current.take() {
                pages.push(page);
            }
            current = Some(BboxPage {
                page_num: pages.len() + 1,
                words: Vec::new(),
            });
            continue;
        }

        if line.contains("</page>") {
            if let Some(page) = current.take() {
                pages.push(page);
            }
            continue;
        }

        let Some(caps) = word_re.captures(line) else {
            continue;
        };
        let Some(page) = current.as_mut() else {
            continue;
        };

        page.words.push(PositionedWord {
            text: unescape_xml(caps.get(5).map_or("", |m| m.as_str())),
            x_min: caps[1].parse()?,
            y_min: caps[2].parse()?,
            x_max: caps[3].parse()?,
            y_max: caps[4].parse()?,
        });
    }

    if let Some(page) = current.take() {
        pages.push(page);
    }

    Ok(pages)
}

fn build_masks_from_pages(pages: &[BboxPage], unsafe_show_secrets: bool) -> AutoMaskResult {
    let detector = Detector::new();
    let mut masks = Vec::new();
    let mut unmapped_findings = 0;
    let mut report = Report::new(pages.len(), unsafe_show_secrets);

    for page in pages {
        let (text, spans) = page_text_and_spans(&page.words);
        for detection in detector.scan_text(&text) {
            let overlapping: Vec<&PositionedWord> = page
                .words
                .iter()
                .zip(spans.iter())
                .filter_map(|(word, (start, end))| {
                    if *start < detection.end && *end > detection.start {
                        Some(word)
                    } else {
                        None
                    }
                })
                .collect();

            if overlapping.is_empty() {
                unmapped_findings += 1;
                continue;
            }

            let x_min = overlapping
                .iter()
                .map(|w| w.x_min)
                .fold(f32::INFINITY, f32::min);
            let y_min = overlapping
                .iter()
                .map(|w| w.y_min)
                .fold(f32::INFINITY, f32::min);
            let x_max = overlapping
                .iter()
                .map(|w| w.x_max)
                .fold(f32::NEG_INFINITY, f32::max);
            let y_max = overlapping
                .iter()
                .map(|w| w.y_max)
                .fold(f32::NEG_INFINITY, f32::max);

            let mask = MaskRegion::from_bbox(page.page_num, x_min, y_min, x_max, y_max);
            masks.push(mask);
            report.add_occurrence_with_source(
                page.page_num,
                &detection.kind,
                Some(detection.value.clone()),
                Some(detection.start),
                Some(detection.end),
                Some(Coords {
                    x: mask.x,
                    y: mask.y,
                    width: mask.width,
                    height: mask.height,
                }),
                mask::apply_mask(&detection.value, &detection.kind, MaskMode::Label),
                "pdf_text_bbox",
            );
        }
    }

    AutoMaskResult {
        masks,
        report,
        unmapped_findings,
    }
}

fn page_text_and_spans(words: &[PositionedWord]) -> (String, Vec<(usize, usize)>) {
    let mut text = String::new();
    let mut spans = Vec::with_capacity(words.len());

    for (idx, word) in words.iter().enumerate() {
        if idx != 0 {
            text.push(' ');
        }
        let start = text.len();
        text.push_str(&word.text);
        let end = text.len();
        spans.push((start, end));
    }

    (text, spans)
}

fn unescape_xml(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_split_phone_and_iban_to_mask_regions() {
        let pages = vec![BboxPage {
            page_num: 1,
            words: vec![
                PositionedWord {
                    text: "Tel".into(),
                    x_min: 10.0,
                    y_min: 10.0,
                    x_max: 20.0,
                    y_max: 20.0,
                },
                PositionedWord {
                    text: "06".into(),
                    x_min: 30.0,
                    y_min: 10.0,
                    x_max: 40.0,
                    y_max: 20.0,
                },
                PositionedWord {
                    text: "11".into(),
                    x_min: 42.0,
                    y_min: 10.0,
                    x_max: 52.0,
                    y_max: 20.0,
                },
                PositionedWord {
                    text: "22".into(),
                    x_min: 54.0,
                    y_min: 10.0,
                    x_max: 64.0,
                    y_max: 20.0,
                },
                PositionedWord {
                    text: "33".into(),
                    x_min: 66.0,
                    y_min: 10.0,
                    x_max: 76.0,
                    y_max: 20.0,
                },
                PositionedWord {
                    text: "44".into(),
                    x_min: 78.0,
                    y_min: 10.0,
                    x_max: 88.0,
                    y_max: 20.0,
                },
                PositionedWord {
                    text: "FR76".into(),
                    x_min: 30.0,
                    y_min: 30.0,
                    x_max: 50.0,
                    y_max: 40.0,
                },
                PositionedWord {
                    text: "3000".into(),
                    x_min: 52.0,
                    y_min: 30.0,
                    x_max: 72.0,
                    y_max: 40.0,
                },
                PositionedWord {
                    text: "6000".into(),
                    x_min: 74.0,
                    y_min: 30.0,
                    x_max: 94.0,
                    y_max: 40.0,
                },
                PositionedWord {
                    text: "0001".into(),
                    x_min: 96.0,
                    y_min: 30.0,
                    x_max: 116.0,
                    y_max: 40.0,
                },
                PositionedWord {
                    text: "1234".into(),
                    x_min: 118.0,
                    y_min: 30.0,
                    x_max: 138.0,
                    y_max: 40.0,
                },
                PositionedWord {
                    text: "5678".into(),
                    x_min: 140.0,
                    y_min: 30.0,
                    x_max: 160.0,
                    y_max: 40.0,
                },
                PositionedWord {
                    text: "90189".into(),
                    x_min: 162.0,
                    y_min: 30.0,
                    x_max: 187.0,
                    y_max: 40.0,
                },
            ],
        }];

        let result = build_masks_from_pages(&pages, false);
        assert_eq!(result.unmapped_findings, 0);
        assert_eq!(result.masks.len(), 2);
        assert!(result.masks.iter().any(|m| m.x <= 30.0 && m.width >= 58.0));
        assert!(result.masks.iter().any(|m| m.x <= 30.0 && m.width >= 157.0));
    }
}
