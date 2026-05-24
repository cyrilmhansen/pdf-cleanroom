/// Flatten module — render PDF pages to images and rebuild an image-only PDF.
///
/// The `flatten-raster` strategy creates a visually faithful image-only PDF
/// by rendering each source page to a PPM raster image via an external
/// renderer (pdftoppm, mutool, or Ghostscript), then embedding those images
/// into a fresh lopdf document.
///
/// # Properties
///
/// - No selectable text (reduces exposure surface)
/// - No source PDF objects copied (metadata, annotations, forms, JS, etc.)
/// - Visual appearance preserved at the renderer's DPI (default 200)
/// - Optional internal pixel masks can black out rendered regions before PDF
///   reconstruction; this is raster redaction, not PDF overlay redaction.
///
/// # Mask coordinates
///
/// `MaskRegion` rectangles use 1-indexed PDF page numbers and PDF user-space
/// points. The origin is the page's bottom-left corner, `x` grows right, and
/// `y` grows up. During raster redaction, rectangles are converted to rendered
/// image pixels by scaling against the source page MediaBox and flipping the
/// Y axis to the image's top-left origin. Regions are clipped to page bounds.
///
/// # Limitations
///
/// - An external PDF renderer must be installed (pdftoppm, mutool, or gs)
/// - Image-only output cannot be searched or copied
/// - Page dimensions are approximated from the source MediaBox or derived
///   from the rendered image dimensions at 200 DPI
/// - Multi-byte text, unusual encodings, and transparency may not survive
///   rendering faithfully in all PDF viewers
use std::fs;
use std::io::BufWriter;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use flate2::write::ZlibEncoder;
use flate2::Compression;
use lopdf::{dictionary, Document, Object, Stream};

static FLATTEN_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Error type for flatten operations.
#[derive(Debug)]
pub enum FlattenError {
    /// No compatible PDF renderer found on PATH.
    NoRenderer,
    /// I/O error during rendering or file operations.
    Io(std::io::Error),
    /// The renderer process failed.
    Render { tool: String, detail: String },
    /// PDF load or structure error.
    Pdf { detail: String },
    /// PPM parsing error.
    Parse { detail: String },
    /// Source PDF has no pages.
    PageCount,
}

/// A rectangular region to black out on a rendered page before PDF rebuild.
///
/// Coordinates are intentionally simple for the raster-redaction foundation:
/// - `page` is 1-indexed.
/// - `x`, `y`, `width`, and `height` are PDF user-space points.
/// - origin is the page's bottom-left corner (`x` right, `y` up).
/// - conversion to pixels scales by rendered image size / page MediaBox size
///   and flips Y into the raster image's top-left origin.
///
/// The `source` and `reason` fields are provenance only. Current masking does
/// not interpret them; future detector/OCR pipelines can use them to explain
/// how a region was derived.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskRegion {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub source: String,
    pub reason: String,
}

impl std::fmt::Display for FlattenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRenderer => {
                write!(
                    f,
                    "no PDF renderer found — install poppler-utils (pdftoppm), \
                     mupdf-tools (mutool), or ghostscript (gs)"
                )
            }
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Render { tool, detail } => {
                write!(f, "{tool} rendering failed: {detail}")
            }
            Self::Pdf { detail } => write!(f, "PDF error: {detail}"),
            Self::Parse { detail } => write!(f, "PPM parse error: {detail}"),
            Self::PageCount => write!(f, "source PDF has no pages"),
        }
    }
}

impl std::error::Error for FlattenError {}

impl From<std::io::Error> for FlattenError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Detect the first available PDF renderer on PATH.
///
/// Returns `Some(name)` where `name` is `"pdftoppm"`, `"mutool"`, or `"gs"`,
/// or `None` if no supported renderer is found.
pub fn detect_renderer() -> Option<String> {
    for (name, arg) in &[
        ("pdftoppm", "--version"),
        ("mutool", "-v"),
        ("gs", "--version"),
    ] {
        if Command::new(name).arg(arg).output().is_ok() {
            return Some(name.to_string());
        }
    }
    None
}

/// Flatten a PDF by rendering each page to an image and rebuilding an
/// image-only PDF.
///
/// # Parameters
///
/// * `input_path` — source PDF file path
/// * `output_path` — destination image-only PDF file path
/// * `renderer` — renderer name (`"pdftoppm"`, `"mutool"`, or `"gs"`)
///
/// # Errors
///
/// Returns `FlattenError::NoRenderer` if no renderer is available.
/// Returns `FlattenError::PageCount` if the source PDF has no pages.
/// Other errors from I/O, rendering, and PDF construction.
pub fn flatten_pdf(
    input_path: &str,
    output_path: &str,
    renderer: &str,
) -> Result<(), FlattenError> {
    flatten_pdf_impl(input_path, output_path, renderer, &[])
}

/// Flatten a PDF with manual raster masks applied before PDF reconstruction.
///
/// This is intentionally not wired to a production CLI yet. It exists as the
/// minimal foundation for real visual redaction: render pages, mutate pixels,
/// then build a fresh image-only PDF. It never overlays rectangles on the
/// source PDF and never copies source PDF text or objects.
pub fn flatten_pdf_with_masks(
    input_path: &str,
    output_path: &str,
    renderer: &str,
    masks: &[MaskRegion],
) -> Result<(), FlattenError> {
    flatten_pdf_impl(input_path, output_path, renderer, masks)
}

fn flatten_pdf_impl(
    input_path: &str,
    output_path: &str,
    renderer: &str,
    masks: &[MaskRegion],
) -> Result<(), FlattenError> {
    // 1. Load source PDF to get page count and dimensions
    let src_doc = Document::load(input_path).map_err(|e| FlattenError::Pdf {
        detail: format!("failed to load source PDF: {e}"),
    })?;
    let num_pages = src_doc.get_pages().len();

    if num_pages == 0 {
        return Err(FlattenError::PageCount);
    }

    let page_dims = extract_page_dimensions(&src_doc, num_pages)?;

    // 2. Create a unique output directory for intermediate PPM files. Tests may
    // run flatten operations concurrently, so a shared directory is unsafe.
    let out_dir = PathBuf::from("target/pdf-cleanroom-flatten").join(format!(
        "{}-{}",
        std::process::id(),
        FLATTEN_TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&out_dir)?;

    // 3. Render each page, apply raster masks, and collect image data
    let mut pages: Vec<(u32, u32, Vec<u8>)> = Vec::with_capacity(num_pages);

    for page_num in 1..=num_pages {
        let ppm_path = render_single_page(renderer, input_path, page_num, &out_dir)?;
        let (w, h, mut data) = parse_ppm(&ppm_path)?;
        let (page_w, page_h) = page_dims
            .get(page_num - 1)
            .copied()
            .unwrap_or((w as f64 * 72.0 / 200.0, h as f64 * 72.0 / 200.0));
        apply_masks_to_page(&mut data, w, h, page_w, page_h, page_num, masks);
        pages.push((w, h, data));
        // Clean up intermediate PPM
        fs::remove_file(&ppm_path).ok();
    }

    // 4. Build image-only PDF
    build_image_pdf(output_path, &pages, &page_dims)?;

    // Clean up output directory
    fs::remove_dir_all(&out_dir).ok();

    Ok(())
}

// ---------------------------------------------------------------------------
// Renderer detection and page rendering
// ---------------------------------------------------------------------------

/// Render a single PDF page to a PPM file.
///
/// Returns the path to the generated `.ppm` file.
fn render_single_page(
    renderer: &str,
    input_path: &str,
    page_num: usize,
    out_dir: &Path,
) -> Result<PathBuf, FlattenError> {
    match renderer {
        "pdftoppm" => {
            let prefix = out_dir.join(format!("page_{:04}", page_num));
            let status = Command::new("pdftoppm")
                .arg("-r")
                .arg("200")
                .arg("-f")
                .arg(page_num.to_string())
                .arg("-l")
                .arg(page_num.to_string())
                .arg(input_path)
                .arg(&prefix)
                .status()
                .map_err(|e| FlattenError::Io(e))?;

            if !status.success() {
                return Err(FlattenError::Render {
                    tool: "pdftoppm".into(),
                    detail: format!("exited with {status:?}"),
                });
            }

            // pdftoppm appends `-{page_num}` to the output prefix
            let ppm = PathBuf::from(format!("{}-{}.ppm", prefix.display(), page_num));
            if !ppm.exists() {
                return Err(FlattenError::Render {
                    tool: "pdftoppm".into(),
                    detail: format!("expected output not found: {}", ppm.display()),
                });
            }
            Ok(ppm)
        }

        "mutool" => {
            let ppm = out_dir.join(format!("page_{:04}.ppm", page_num));
            let status = Command::new("mutool")
                .arg("draw")
                .arg("-o")
                .arg(&ppm)
                .arg("-r")
                .arg("200")
                .arg(input_path)
                .arg(format!("{}-{}", page_num, page_num))
                .status()
                .map_err(|e| FlattenError::Io(e))?;

            if !status.success() {
                return Err(FlattenError::Render {
                    tool: "mutool".into(),
                    detail: format!("exited with {status:?}"),
                });
            }

            if !ppm.exists() {
                return Err(FlattenError::Render {
                    tool: "mutool".into(),
                    detail: format!("expected output not found: {}", ppm.display()),
                });
            }
            Ok(ppm)
        }

        "gs" => {
            let ppm = out_dir.join(format!("page_{:04}.ppm", page_num));
            let status = Command::new("gs")
                .arg("-dNOPAUSE")
                .arg("-dBATCH")
                .arg("-dSAFER")
                .arg("-sDEVICE=ppmraw")
                .arg("-r200")
                .arg("-dFirstPage")
                .arg(page_num.to_string())
                .arg("-dLastPage")
                .arg(page_num.to_string())
                .arg("-sOutputFile")
                .arg(&ppm)
                .arg(input_path)
                .status()
                .map_err(|e| FlattenError::Io(e))?;

            if !status.success() {
                return Err(FlattenError::Render {
                    tool: "gs".into(),
                    detail: format!("exited with {status:?}"),
                });
            }

            if !ppm.exists() {
                return Err(FlattenError::Render {
                    tool: "gs".into(),
                    detail: format!("expected output not found: {}", ppm.display()),
                });
            }
            Ok(ppm)
        }

        other => Err(FlattenError::Render {
            tool: other.into(),
            detail: "unknown renderer".into(),
        }),
    }
}

// ---------------------------------------------------------------------------
// PPM parsing
// ---------------------------------------------------------------------------

/// Parse a PPM P6 binary file and return `(width, height, pixel_data)`.
///
/// Pixel data is row-major RGB with 3 bytes per pixel.
fn parse_ppm(path: &Path) -> Result<(u32, u32, Vec<u8>), FlattenError> {
    let data = fs::read(path)?;

    if data.len() < 3 || data[0] != b'P' || data[1] != b'6' || data[2] != b'\n' {
        return Err(FlattenError::Parse {
            detail: "not a valid PPM P6 file".into(),
        });
    }

    // Current position within the file
    let mut pos = 3; // after "P6\n"

    // Skip comment lines (lines starting with #)
    loop {
        // Skip leading whitespace/newlines between header fields
        while pos < data.len() && (data[pos] == b'\n' || data[pos] == b'\r') {
            pos += 1;
        }
        if pos >= data.len() {
            return Err(FlattenError::Parse {
                detail: "unexpected end of PPM header".into(),
            });
        }
        if data[pos] == b'#' {
            // Skip comment line
            while pos < data.len() && data[pos] != b'\n' {
                pos += 1;
            }
            pos += 1; // skip \n
        } else {
            break;
        }
    }

    // Read dimensions: "<width> <height>\n"
    let dim_start = pos;
    while pos < data.len() && data[pos] != b'\n' {
        pos += 1;
    }
    if pos >= data.len() {
        return Err(FlattenError::Parse {
            detail: "unexpected end of PPM dimensions line".into(),
        });
    }
    let dim_line = std::str::from_utf8(&data[dim_start..pos]).map_err(|_| FlattenError::Parse {
        detail: "non-UTF-8 in PPM header".into(),
    })?;
    let parts: Vec<&str> = dim_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(FlattenError::Parse {
            detail: "invalid PPM dimensions line".into(),
        });
    }
    let w: u32 = parts[0].parse().map_err(|_| FlattenError::Parse {
        detail: "invalid PPM width".into(),
    })?;
    let h: u32 = parts[1].parse().map_err(|_| FlattenError::Parse {
        detail: "invalid PPM height".into(),
    })?;
    pos += 1; // skip \n

    // Skip comment lines before maxval
    loop {
        while pos < data.len() && (data[pos] == b'\n' || data[pos] == b'\r') {
            pos += 1;
        }
        if pos >= data.len() {
            break;
        }
        if data[pos] == b'#' {
            while pos < data.len() && data[pos] != b'\n' {
                pos += 1;
            }
            pos += 1;
        } else {
            break;
        }
    }

    // Read maxval line
    while pos < data.len() && data[pos] != b'\n' {
        pos += 1;
    }
    if pos < data.len() {
        pos += 1; // skip \n
    }

    // Binary data starts at pos
    let expected_len = w as usize * h as usize * 3;
    if data.len() - pos < expected_len {
        return Err(FlattenError::Parse {
            detail: format!(
                "PPM data too short: need {expected_len} bytes, got {}",
                data.len() - pos
            ),
        });
    }

    Ok((w, h, data[pos..pos + expected_len].to_vec()))
}

/// Apply black rectangular masks to one rendered RGB page in-place.
///
/// `pixels` is row-major RGB (`width * height * 3`). The PDF coordinate
/// system is bottom-left origin in points; the raster coordinate system is
/// top-left origin in pixels.
fn apply_masks_to_page(
    pixels: &mut [u8],
    image_width: u32,
    image_height: u32,
    page_width: f64,
    page_height: f64,
    page_num: usize,
    masks: &[MaskRegion],
) {
    if image_width == 0 || image_height == 0 || page_width <= 0.0 || page_height <= 0.0 {
        return;
    }

    let expected_len = image_width as usize * image_height as usize * 3;
    if pixels.len() < expected_len {
        return;
    }

    let scale_x = image_width as f64 / page_width;
    let scale_y = image_height as f64 / page_height;

    for mask in masks.iter().filter(|mask| mask.page == page_num) {
        if mask.width <= 0.0 || mask.height <= 0.0 {
            continue;
        }

        let x0 = (mask.x as f64 * scale_x).floor().max(0.0) as u32;
        let x1 = ((mask.x + mask.width) as f64 * scale_x)
            .ceil()
            .clamp(0.0, image_width as f64) as u32;

        // PDF y grows upward from bottom-left; image y grows downward from top-left.
        let y0_pdf = mask.y as f64;
        let y1_pdf = (mask.y + mask.height) as f64;
        let y0 = ((page_height - y1_pdf) * scale_y)
            .floor()
            .clamp(0.0, image_height as f64) as u32;
        let y1 = ((page_height - y0_pdf) * scale_y)
            .ceil()
            .clamp(0.0, image_height as f64) as u32;

        if x0 >= x1 || y0 >= y1 {
            continue;
        }

        for y in y0..y1 {
            let row_start = y as usize * image_width as usize * 3;
            for x in x0..x1 {
                let offset = row_start + x as usize * 3;
                pixels[offset] = 0;
                pixels[offset + 1] = 0;
                pixels[offset + 2] = 0;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// New PDF construction
// ---------------------------------------------------------------------------

/// Extract page dimensions (width, height) from a loaded lopdf document.
///
/// Falls back to `(595.0, 842.0)` (A4) if a page has no MediaBox or the
/// MediaBox cannot be parsed.
fn extract_page_dimensions(
    doc: &Document,
    num_pages: usize,
) -> Result<Vec<(f64, f64)>, FlattenError> {
    let pages_map = doc.get_pages();

    let mut dims: Vec<(f64, f64)> = Vec::with_capacity(num_pages);

    for (expected_page_num, page_id) in &pages_map {
        // Keep mapping correct even if page numbers are non-contiguous
        let pad_to = *expected_page_num as usize;
        while dims.len() < pad_to {
            dims.push((595.0, 842.0));
        }

        let dim = match doc.get_object(*page_id) {
            Ok(page_obj) => match page_obj.as_dict() {
                Ok(dict) => match dict.get(b"MediaBox") {
                    Ok(media_box) => match media_box.as_array() {
                        Ok(arr) if arr.len() >= 4 => {
                            let x0 = arr[0].as_i64().unwrap_or(0) as f64;
                            let y0 = arr[1].as_i64().unwrap_or(0) as f64;
                            let x1 = arr[2].as_i64().unwrap_or(842) as f64;
                            let y1 = arr[3].as_i64().unwrap_or(595) as f64;
                            (x1 - x0, y1 - y0)
                        }
                        _ => (595.0, 842.0),
                    },
                    _ => (595.0, 842.0),
                },
                _ => (595.0, 842.0),
            },
            _ => (595.0, 842.0),
        };
        dims.push(dim);
    }

    // Pad to num_pages
    while dims.len() < num_pages {
        dims.push((595.0, 842.0));
    }

    Ok(dims)
}

/// Build a new image-only PDF from rendered page image data.
///
/// Each page embeds its image as a DeviceRGB XObject scaled to fill the
/// page's MediaBox.  The document is a fresh lopdf structure — no source
/// PDF objects are copied.
fn build_image_pdf(
    output_path: &str,
    pages: &[(u32, u32, Vec<u8>)],
    page_dims: &[(f64, f64)],
) -> Result<(), FlattenError> {
    let mut doc = Document::with_version("1.5");

    // Pages tree node
    let pages_id = doc.new_object_id();
    let mut kid_ids = Vec::new();

    for (idx, (img_w, img_h, data)) in pages.iter().enumerate() {
        let (page_w, page_h) = page_dims
            .get(idx)
            .copied()
            .unwrap_or((*img_w as f64 * 72.0 / 200.0, *img_h as f64 * 72.0 / 200.0));

        // Image XObject — compressed DeviceRGB pixels (FlateDecode)
        let compressed = {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data).map_err(|e| FlattenError::Io(e))?;
            encoder.finish().map_err(|e| FlattenError::Io(e))?
        };
        let image_id = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => Object::from(*img_w as i64),
                "Height" => Object::from(*img_h as i64),
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8_i64,
                "Filter" => "FlateDecode",
            },
            compressed,
        ));

        // Resources dict
        let resources_id = doc.add_object(dictionary! {
            "XObject" => dictionary! {
                "Im0" => image_id,
            },
        });

        // Content stream — scale the image from unit square to fill page
        let content = format!("q\n{:.2} 0 0 {:.2} 0 0 cm\n/Im0 Do\nQ\n", page_w, page_h,);
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));

        // Page object
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => Object::Array(vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(page_w as f32),
                Object::Real(page_h as f32),
            ]),
        });

        kid_ids.push(page_id);
    }

    // Pages tree
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(kid_ids.into_iter().map(Object::from).collect()),
            "Count" => pages.len() as i64,
        }),
    );

    // Catalog
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    // Save to file
    let file = fs::File::create(output_path)?;
    doc.save_to(&mut BufWriter::new(file))
        .map_err(|e| FlattenError::Pdf {
            detail: format!("failed to save output PDF: {e}"),
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb_at(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 3] {
        let offset = (y as usize * width as usize + x as usize) * 3;
        [pixels[offset], pixels[offset + 1], pixels[offset + 2]]
    }

    #[test]
    fn mask_region_blacks_expected_pixels_with_y_axis_flip() {
        let mut pixels = vec![255_u8; 10 * 10 * 3];
        let masks = [MaskRegion {
            page: 1,
            x: 2.0,
            y: 3.0,
            width: 4.0,
            height: 2.0,
            source: "manual".to_string(),
            reason: "unit-test".to_string(),
        }];

        apply_masks_to_page(&mut pixels, 10, 10, 10.0, 10.0, 1, &masks);

        assert_eq!(rgb_at(&pixels, 10, 2, 5), [0, 0, 0]);
        assert_eq!(rgb_at(&pixels, 10, 5, 6), [0, 0, 0]);
        assert_eq!(rgb_at(&pixels, 10, 1, 5), [255, 255, 255]);
        assert_eq!(rgb_at(&pixels, 10, 2, 4), [255, 255, 255]);
        assert_eq!(rgb_at(&pixels, 10, 2, 7), [255, 255, 255]);
    }

    #[test]
    fn mask_region_ignores_other_pages() {
        let mut pixels = vec![255_u8; 4 * 4 * 3];
        let masks = [MaskRegion {
            page: 2,
            x: 0.0,
            y: 0.0,
            width: 4.0,
            height: 4.0,
            source: "manual".to_string(),
            reason: "unit-test".to_string(),
        }];

        apply_masks_to_page(&mut pixels, 4, 4, 4.0, 4.0, 1, &masks);

        assert!(pixels.iter().all(|channel| *channel == 255));
    }
}
