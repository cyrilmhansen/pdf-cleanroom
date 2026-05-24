//! PDF fixture generators for integration tests.
//!
//! All PDFs are created at runtime using lopdf for low-level PDF construction
//! or printpdf for simple text documents. No binary fixtures are committed.

use std::io::BufWriter;
use ::lopdf::{dictionary, Document, Object, ObjectId, Stream};
use printpdf::*;

/// The secrets we plant in test PDFs.
pub const SECRET_EMAIL: &str = "jean.dupont@example.com";
pub const SECRET_PHONE: &str = "06 11 22 33 44";
pub const SECRET_PHONE_COMPACT: &str = "0611223344";
pub const SECRET_IBAN: &str = "FR7630006000011234567890189";

// ---------------------------------------------------------------------------
// 1. Visible text secrets (printpdf)
// ---------------------------------------------------------------------------

pub fn visible_text_pdf() -> Vec<u8> {
    let (doc, page, layer) = PdfDocument::new(
        "visible-secrets",
        Mm(210.0),
        Mm(297.0),
        "Content",
    );
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .expect("font");
    let layer = doc.get_page(page).get_layer(layer);

    layer.use_text(
        &format!("Email: {SECRET_EMAIL}"),
        11.0, Mm(20.0), Mm(270.0), &font,
    );
    layer.use_text(
        &format!("Tel: {SECRET_PHONE}"),
        11.0, Mm(20.0), Mm(260.0), &font,
    );
    layer.use_text(
        &format!("IBAN: {SECRET_IBAN}"),
        11.0, Mm(20.0), Mm(250.0), &font,
    );

    let mut buf = Vec::new();
    doc.save(&mut BufWriter::new(&mut buf))
        .expect("save visible-text PDF");
    buf
}

// ---------------------------------------------------------------------------
// Helper: build a skeleton lopdf document with one page
// ---------------------------------------------------------------------------

fn new_doc() -> (Document, ObjectId, ObjectId, ObjectId) {
    let mut doc = Document::with_version("1.5");

    let info_id = doc.add_object(dictionary! {
        "Creator" => Object::string_literal("pdf-cleanroom test suite"),
    });
    doc.trailer.set("Info", info_id);

    // Font
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // Resources
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_id,
        },
    });

    // Empty content
    let content_text = b"BT /F1 12 Tf 50 100 Td () Tj ET\n";
    let content_id = doc.add_object(Stream::new(
        dictionary! {},
        content_text.to_vec(),
    ));

    // Pages tree
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });

    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::from(page_id)],
        "Count" => 1,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    // Catalog
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    (doc, catalog_id, page_id, content_id)
}

/// Store text content as raw PDF operators on a page.
fn set_page_content(doc: &mut Document, page_id: ObjectId, content_text: &str) {
    let content_id = doc.add_object(Stream::new(
        dictionary! {},
        content_text.as_bytes().to_vec(),
    ));
    if let Ok(page) = doc.get_dictionary_mut(page_id) {
        page.set("Contents", content_id);
    }
}

/// Save a lopdf document to bytes.
fn save_doc(doc: &mut Document) -> Vec<u8> {
    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("save doc");
    buf
}

// ---------------------------------------------------------------------------
// 2. Metadata secrets (lopdf)
// ---------------------------------------------------------------------------

pub fn metadata_secrets_pdf() -> Vec<u8> {
    let (mut doc, _catalog_id, _page_id, _content_id) = new_doc();

    let info_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal(format!("Contact: {SECRET_EMAIL}")),
        "Subject" => Object::string_literal(format!("Phone: {SECRET_PHONE}")),
        "Author" => Object::string_literal(format!("IBAN: {SECRET_IBAN}")),
        "Creator" => Object::string_literal("pdf-cleanroom test suite"),
        "Producer" => Object::string_literal("pdf-cleanroom test suite"),
    });
    doc.trailer.set("Info", info_id);

    save_doc(&mut doc)
}

// ---------------------------------------------------------------------------
// 3. XMP metadata secrets (lopdf)
// ---------------------------------------------------------------------------

pub fn xmp_metadata_pdf() -> Vec<u8> {
    let (mut doc, catalog_id, _page_id, _content_id) = new_doc();

    let xmp_xml = format!(
        r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
        xmlns:dc="http://purl.org/dc/elements/1.1/">
      <dc:description>{email}, {phone}, {iban}</dc:description>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#,
        email = SECRET_EMAIL,
        phone = SECRET_PHONE,
        iban = SECRET_IBAN,
    );

    let xmp_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "Metadata",
            "Subtype" => "XML",
        },
        xmp_xml.into_bytes(),
    ));

    if let Ok(catalog) = doc.get_dictionary_mut(catalog_id) {
        catalog.set("Metadata", xmp_id);
    }

    save_doc(&mut doc)
}

// ---------------------------------------------------------------------------
// 4. Embedded file attachments (lopdf)
// ---------------------------------------------------------------------------

pub fn embedded_file_pdf() -> Vec<u8> {
    let (mut doc, catalog_id, _page_id, _content_id) = new_doc();

    let attachment_content = format!(
        "Secret data: {email}\nPhone: {phone}\nIBAN: {iban}",
        email = SECRET_EMAIL,
        phone = SECRET_PHONE,
        iban = SECRET_IBAN,
    );
    let embedded_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "EmbeddedFile",
            "Subtype" => "application/octet-stream",
            "Params" => dictionary! {
                "Size" => attachment_content.len() as i64,
            },
        },
        attachment_content.into_bytes(),
    ));

    let filespec_id = doc.add_object(dictionary! {
        "Type" => "Filespec",
        "F" => Object::string_literal("secrets.txt"),
        "UF" => Object::string_literal("secrets.txt"),
        "EF" => dictionary! {
            "F" => embedded_id,
            "UF" => embedded_id,
        },
    });

    let names_id = doc.add_object(dictionary! {
        "EmbeddedFiles" => dictionary! {
            "Names" => vec![
                Object::string_literal("secrets.txt"),
                Object::from(filespec_id),
            ],
        },
    });

    if let Ok(catalog) = doc.get_dictionary_mut(catalog_id) {
        catalog.set("Names", names_id);
    }

    save_doc(&mut doc)
}

// ---------------------------------------------------------------------------
// 5. Annotations with secrets (lopdf)
// ---------------------------------------------------------------------------

pub fn annotation_secrets_pdf() -> Vec<u8> {
    let (mut doc, _catalog_id, page_id, _content_id) = new_doc();

    let annot_id = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Text",
        "Contents" => Object::string_literal(format!(
            "Secret annotation: {email}\nPhone: {phone}\nIBAN: {iban}",
            email = SECRET_EMAIL,
            phone = SECRET_PHONE,
            iban = SECRET_IBAN,
        )),
        "Rect" => vec![
            Object::from(10.0_f32),
            Object::from(10.0_f32),
            Object::from(100.0_f32),
            Object::from(50.0_f32),
        ],
    });

    if let Ok(page) = doc.get_dictionary_mut(page_id) {
        page.set("Annots", vec![Object::from(annot_id)]);
    }

    save_doc(&mut doc)
}

// ---------------------------------------------------------------------------
// 6. Form fields / AcroForm with secrets (lopdf)
// ---------------------------------------------------------------------------

pub fn form_field_secrets_pdf() -> Vec<u8> {
    let (mut doc, catalog_id, page_id, _content_id) = new_doc();

    let font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    };
    let form_font_id = doc.add_object(font_dict);

    let field_id = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "FT" => "Tx",
        "T" => Object::string_literal("email_field"),
        "V" => Object::string_literal(SECRET_EMAIL),
        "DV" => Object::string_literal(SECRET_EMAIL),
        "DA" => Object::string_literal(format!("/F1 {:.1} Tf 0 g", 10.0)),
        "Rect" => vec![10.0_f32.into(), 10.0_f32.into(), 200.0_f32.into(), 30.0_f32.into()],
        "P" => Object::from(page_id),
    });

    if let Ok(page) = doc.get_dictionary_mut(page_id) {
        page.set("Annots", vec![Object::from(field_id)]);
    }

    let form_id = doc.add_object(dictionary! {
        "Fields" => vec![Object::from(field_id)],
        "DR" => dictionary! {
            "Font" => dictionary! {
                "F1" => form_font_id,
            },
        },
    });

    if let Ok(catalog) = doc.get_dictionary_mut(catalog_id) {
        catalog.set("AcroForm", form_id);
    }

    save_doc(&mut doc)
}

// ---------------------------------------------------------------------------
// 7. Hidden / non-visible text
// ---------------------------------------------------------------------------

/// White text on white background.
pub fn hidden_white_text_pdf() -> Vec<u8> {
    let (mut doc, _catalog_id, page_id, _content_id) = new_doc();
    set_page_content(
        &mut doc,
        page_id,
        &format!(
            "BT /F1 12 Tf 1 1 1 rg 50 200 Td ({email}) Tj 0 0 0 rg ET\n\
             BT /F1 12 Tf 50 100 Td (Visible text) Tj ET\n",
            email = SECRET_EMAIL
        ),
    );
    save_doc(&mut doc)
}

/// Text outside visible page bounds (negative X).
pub fn text_outside_bounds_pdf() -> Vec<u8> {
    let (mut doc, _catalog_id, page_id, _content_id) = new_doc();
    set_page_content(
        &mut doc,
        page_id,
        &format!(
            "BT /F1 12 Tf -100 200 Td ({email}) Tj ET\n\
             BT /F1 12 Tf 50 100 Td (Visible text) Tj ET\n",
            email = SECRET_EMAIL
        ),
    );
    save_doc(&mut doc)
}

/// Very small text (1pt font).
pub fn tiny_text_pdf() -> Vec<u8> {
    let (mut doc, _catalog_id, page_id, _content_id) = new_doc();
    set_page_content(
        &mut doc,
        page_id,
        &format!(
            "BT /F1 1 Tf 50 200 Td ({email}) Tj ET\n\
             BT /F1 12 Tf 50 100 Td (Visible text) Tj ET\n",
            email = SECRET_EMAIL
        ),
    );
    save_doc(&mut doc)
}

// ---------------------------------------------------------------------------
// 8. Text covered by opaque shape
// ---------------------------------------------------------------------------

pub fn text_covered_by_shape_pdf() -> Vec<u8> {
    let (mut doc, _catalog_id, page_id, _content_id) = new_doc();
    set_page_content(
        &mut doc,
        page_id,
        &format!(
            "BT /F1 12 Tf 50 200 Td ({email}) Tj ET\n\
             q 0 0 0 rg 40 190 200 20 re f Q\n\
             BT /F1 12 Tf 50 100 Td (Visible text) Tj ET\n",
            email = SECRET_EMAIL
        ),
    );
    save_doc(&mut doc)
}

// ---------------------------------------------------------------------------
// 9. Fragmented secrets (split across text chunks)
// ---------------------------------------------------------------------------

pub fn fragmented_secrets_pdf() -> Vec<u8> {
    let (mut doc, _catalog_id, page_id, _content_id) = new_doc();
    set_page_content(
        &mut doc,
        page_id,
        &format!(
            // Fragmented email across Td operations
            "BT /F1 12 Tf 50 250 Td (cy) Tj 15 0 Td (ril@) Tj 20 0 Td (examp) Tj 30 0 Td (le.com) Tj ET\n\
             // Fragmented IBAN
             BT /F1 12 Tf 50 230 Td (FR76) Tj 30 0 Td (3000) Tj 30 0 Td (6000) Tj 30 0 Td (0112) Tj \
             30 0 Td (3456) Tj 30 0 Td (7890) Tj 30 0 Td (189) Tj ET\n\
             // Fragmented phone
             BT /F1 12 Tf 50 210 Td (06) Tj 20 0 Td (11) Tj 20 0 Td (22) Tj 20 0 Td (33) Tj 20 0 Td (44) Tj ET\n\
             BT /F1 12 Tf 50 100 Td (Visible text) Tj ET\n"
        ),
    );
    save_doc(&mut doc)
}

// ---------------------------------------------------------------------------
// 10. Image-only PDF (no text content)
// ---------------------------------------------------------------------------

pub fn image_only_pdf() -> Vec<u8> {
    let (mut doc, _catalog_id, page_id, _content_id) = new_doc();
    set_page_content(
        &mut doc,
        page_id,
        "q 1 1 1 rg 0 0 595 842 re f Q\n\
         q 0.5 0.5 1 rg 100 300 200 200 re f Q\n",
    );
    save_doc(&mut doc)
}

pub fn image_with_text_pdf() -> Vec<u8> {
    let (mut doc, _catalog_id, page_id, _content_id) = new_doc();
    set_page_content(
        &mut doc,
        page_id,
        "q 0.8 0.8 0.8 rg 50 150 300 200 re f Q\n\
         BT /F1 12 Tf 50 100 Td (This is image caption with no secrets) Tj ET\n",
    );
    save_doc(&mut doc)
}

// ---------------------------------------------------------------------------
// Helper: PDF with a real embedded image (raw bitmap XObject)
// ---------------------------------------------------------------------------

/// Generate a small 2×2 RGB bitmap as raw pixel bytes.
fn tiny_rgb_bitmap() -> Vec<u8> {
    vec![
        255, 0, 0,    // pixel 0: red
        0, 255, 0,    // pixel 1: green
        0, 0, 255,    // pixel 2: blue
        255, 255, 255, // pixel 3: white
    ]
}

/// Build a lopdf Document skeleton that includes a real embedded image XObject
/// and optionally visible text content.
fn doc_with_embedded_image(
    secret_text: Option<&str>,
) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");

    // Image stream: tiny raw RGB bitmap
    let image_data = tiny_rgb_bitmap();
    let image_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 2_i64,
            "Height" => 2_i64,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8_i64,
        },
        image_data,
    ));

    // Font
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // Resources: font + image
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_id,
        },
        "XObject" => dictionary! {
            "Im0" => image_id,
        },
    });

    // Page content: draw image, optionally overlay text
    let content_text = if let Some(text) = secret_text {
        format!("q\n/Im0 Do\nQ\nBT /F1 12 Tf 50 50 Td ({text}) Tj ET\n")
    } else {
        "q\n/Im0 Do\nQ\n".to_string()
    };
    let content_id = doc.add_object(Stream::new(
        dictionary! {},
        content_text.into_bytes(),
    ));

    // Pages tree
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });

    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::from(page_id)],
        "Count" => 1,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    // Catalog
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("save image PDF");
    buf
}

/// PDF with a real embedded image, no text content.
/// Useful for OCR tests — secrets could be in the image but not in extracted text.
pub fn image_embedded_only_pdf() -> Vec<u8> {
    doc_with_embedded_image(None)
}

/// PDF with a real embedded image AND visible text (no secrets in text).
/// The text is a caption, not a secret. Secrets in the image are invisible to scan.
pub fn image_embedded_with_text_pdf() -> Vec<u8> {
    doc_with_embedded_image(Some("Image caption"))
}

// ---------------------------------------------------------------------------
// 16. Accented text PDF (no secrets, French accents)
// ---------------------------------------------------------------------------

/// Multi-line PDF with accented French characters (é, è, ê, à, ç, ô, etc.).
/// No secrets — used to verify text-only rebuild preserves Unicode.
pub fn accented_text_pdf() -> Vec<u8> {
    let (doc, page, layer) = PdfDocument::new(
        "accented-text",
        Mm(210.0),
        Mm(297.0),
        "Content",
    );
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .expect("font");
    let layer = doc.get_page(page).get_layer(layer);

    layer.use_text("Français: é è ê ë à â ä ç ô î ï û ü", 11.0, Mm(20.0), Mm(270.0), &font);
    layer.use_text("Español: ñ ó í ú ü á é", 11.0, Mm(20.0), Mm(260.0), &font);
    layer.use_text("Deutsch: ä ö ü ß Ä Ö Ü", 11.0, Mm(20.0), Mm(250.0), &font);
    layer.use_text("Symboles: € « » — …", 11.0, Mm(20.0), Mm(240.0), &font);
    layer.use_text("Chiffres: 1 250,00 € — 99,9 %", 11.0, Mm(20.0), Mm(230.0), &font);

    let mut buf = Vec::new();
    doc.save(&mut BufWriter::new(&mut buf))
        .expect("save accented-text PDF");
    buf
}

// ---------------------------------------------------------------------------
// 17. Synthetic administrative document PDF (has secrets)
// ---------------------------------------------------------------------------

/// PDF resembling a bank transfer certificate with headings, labels, amounts,
/// and embedded secrets (IBAN, phone, email).
///
/// Secrets are in visible text and should be detected and removed by rebuild.
/// Labels and structural text should survive.
pub fn admin_document_pdf() -> Vec<u8> {
    let (doc, page, layer) = PdfDocument::new(
        "releve-de-compte",
        Mm(210.0),
        Mm(297.0),
        "Content",
    );
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .expect("font");
    let layer = doc.get_page(page).get_layer(layer);

    // Heading
    layer.use_text("RELEVÉ DE COMPTE", 14.0, Mm(20.0), Mm(275.0), &font);
    layer.use_text("Banque Fictive SA — Paris", 10.0, Mm(20.0), Mm(268.0), &font);
    layer.use_text("Date: 15/04/2026", 10.0, Mm(20.0), Mm(260.0), &font);

    // Account info
    layer.use_text("Titulaire: Jean Démo", 11.0, Mm(20.0), Mm(248.0), &font);
    layer.use_text("IBAN: FR76 3000 6000 0112 3456 7890 189", 11.0, Mm(20.0), Mm(238.0), &font);

    // Contact
    layer.use_text("Email: jean.demo@example.com", 11.0, Mm(20.0), Mm(226.0), &font);
    layer.use_text("Tél: 06 11 22 33 44", 11.0, Mm(20.0), Mm(216.0), &font);

    // Transaction
    layer.use_text("Virement entrant — 1 250,00 €", 11.0, Mm(20.0), Mm(204.0), &font);
    layer.use_text("Référence: VIR-2026-0042", 10.0, Mm(20.0), Mm(196.0), &font);
    layer.use_text("Motif: Remboursement prêt", 10.0, Mm(20.0), Mm(188.0), &font);
    layer.use_text("Solde après opération: 4 820,75 €", 11.0, Mm(20.0), Mm(178.0), &font);

    let mut buf = Vec::new();
    doc.save(&mut BufWriter::new(&mut buf))
        .expect("save admin-document PDF");
    buf
}

// ---------------------------------------------------------------------------
// 18. Image-only PDF with secret text rendered into pixels (for OCR tests)
// ---------------------------------------------------------------------------

/// 5×7 bitmap font data.
///
/// Each character is 7 rows. Each byte's lower 5 bits represent one row:
/// bit 4 = leftmost column, bit 0 = rightmost column.
fn bitmap_5x7(c: u8) -> [u8; 7] {
    match c {
        b' ' => [0x00; 7],
        b'0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        b'1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        b'2' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        b'3' => [0x1F, 0x01, 0x02, 0x06, 0x01, 0x11, 0x0E],
        b'4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        b'5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        b'6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        b'7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        b'8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        b'9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        b'a' => [0x00, 0x00, 0x0E, 0x01, 0x0F, 0x11, 0x0F],
        b'b' => [0x10, 0x10, 0x1E, 0x11, 0x11, 0x11, 0x1E],
        b'c' => [0x00, 0x00, 0x0E, 0x11, 0x10, 0x11, 0x0E],
        b'd' => [0x01, 0x01, 0x0F, 0x11, 0x11, 0x11, 0x0F],
        b'e' => [0x00, 0x00, 0x0E, 0x11, 0x1F, 0x10, 0x0E],
        b'f' => [0x06, 0x09, 0x08, 0x1E, 0x08, 0x08, 0x08],
        b'g' => [0x00, 0x00, 0x0F, 0x11, 0x11, 0x0F, 0x01],
        b'h' => [0x10, 0x10, 0x1E, 0x11, 0x11, 0x11, 0x11],
        b'i' => [0x04, 0x00, 0x0C, 0x04, 0x04, 0x04, 0x0E],
        b'j' => [0x02, 0x00, 0x06, 0x02, 0x02, 0x12, 0x0C],
        b'l' => [0x0C, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        b'm' => [0x00, 0x00, 0x1A, 0x15, 0x15, 0x15, 0x15],
        b'n' => [0x00, 0x00, 0x1E, 0x11, 0x11, 0x11, 0x11],
        b'o' => [0x00, 0x00, 0x0E, 0x11, 0x11, 0x11, 0x0E],
        b'p' => [0x00, 0x00, 0x1E, 0x11, 0x11, 0x1E, 0x10],
        b'r' => [0x00, 0x00, 0x16, 0x19, 0x10, 0x10, 0x10],
        b's' => [0x00, 0x00, 0x0E, 0x10, 0x0E, 0x01, 0x1E],
        b't' => [0x08, 0x08, 0x1E, 0x08, 0x08, 0x09, 0x06],
        b'u' => [0x00, 0x00, 0x11, 0x11, 0x11, 0x11, 0x0E],
        b'v' => [0x00, 0x00, 0x11, 0x11, 0x0A, 0x0A, 0x04],
        b'x' => [0x00, 0x00, 0x11, 0x0A, 0x04, 0x0A, 0x11],
        b'F' => [0x1F, 0x10, 0x1F, 0x10, 0x10, 0x10, 0x10],
        b'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        b'.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        b'@' => [0x0E, 0x11, 0x17, 0x15, 0x17, 0x10, 0x0E],
        _   => [0x00; 7],
    }
}

/// Render a string into a raw RGB buffer using a 5×7 bitmap font.
///
/// Returns `(pixels, width, height)` where `pixels` has `width × height × 3`
/// bytes (RGB, row-major, top-to-bottom). Background is white, text is black.
///
/// `scale` controls how many actual pixels each font pixel occupies (e.g. 4 →
/// each 5×7 character cell becomes 20×28 pixels).
fn render_text_to_rgb(
    text: &str,
    scale: u32,
) -> (Vec<u8>, u32, u32) {
    // Character cell metrics (in font-pixel units)
    const CHAR_W: u32 = 5;
    const CHAR_H: u32 = 7;
    const SPACE_X: u32 = 1; // horizontal gap between chars
    const SPACE_Y: u32 = 2; // vertical gap between lines

    // Layout text into lines (split on '\n')
    let lines: Vec<&str> = text.lines().collect();
    let max_line_len = lines.iter().map(|l| l.len()).max().unwrap_or(0);

    // Image dimensions in actual pixels
    let img_w = ((CHAR_W + SPACE_X) * max_line_len as u32 + SPACE_X) * scale;
    let img_h = ((CHAR_H + SPACE_Y) * lines.len() as u32 + SPACE_Y) * scale;
    let img_w_usize = img_w as usize;
    let img_h_usize = img_h as usize;

    // Allocate RGB buffer, fill with white
    let mut pixels = vec![255u8; img_w_usize * img_h_usize * 3];

    for (line_idx, line) in lines.iter().enumerate() {
        let y0 = (SPACE_Y + (CHAR_H + SPACE_Y) * line_idx as u32) * scale;
        for (col, ch) in line.bytes().enumerate() {
            let x0 = (SPACE_X + (CHAR_W + SPACE_X) * col as u32) * scale;
            let bitmap = bitmap_5x7(ch);

            for row in 0..CHAR_H {
                let row_bits = bitmap[row as usize];
                for col_bit in 0..CHAR_W {
                    // Bit 4 = leftmost column
                    if (row_bits >> (4 - col_bit)) & 1 == 0 {
                        continue; // background pixel, stays white
                    }
                    // Draw a scaled block of black pixels
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = (x0 + col_bit * scale + sx) as usize;
                            let py = (y0 + row * scale + sy) as usize;
                            let idx = (py * img_w_usize + px) * 3;
                            pixels[idx..idx + 3].copy_from_slice(&[0, 0, 0]);
                        }
                    }
                }
            }
        }
    }

    (pixels, img_w, img_h)
}

/// PDF containing only an image with visible secret text rendered into pixels.
///
/// The secrets (email, phone, IBAN) are drawn as raster graphics — they do not
/// exist as selectable PDF text. Normal text extraction will find nothing.
/// Only OCR on a rendered PNG can recover them.
pub fn image_only_secrets_pdf() -> Vec<u8> {
    let (ppm, w, h) = render_secrets_ppm();
    // Skip PPM header to get raw RGB data
    let header_end = ppm.iter().position(|&b| b == b'\n')
        .and_then(|i1| ppm[i1+1..].iter().position(|&b| b == b'\n'))
        .and_then(|i2| ppm[i2+1..].iter().position(|&b| b == b'\n'))
        .map(|i3| i3 + 3)
        .expect("valid PPM header");
    let image_data = ppm[header_end..].to_vec();
    let img_w = w;
    let img_h = h;

    let mut doc = Document::with_version("1.5");

    // Image XObject
    let image_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => Object::from(img_w as i64),
            "Height" => Object::from(img_h as i64),
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8_i64,
        },
        image_data,
    ));

    // Resources: just the image
    let resources_id = doc.add_object(dictionary! {
        "XObject" => dictionary! {
            "Im0" => image_id,
        },
    });

    // Page content: scale image from unit square to its pixel dimensions
    let content_text = format!(
        "q\n{} 0 0 {} 20 20 cm\n/Im0 Do\nQ\n",
        img_w, img_h,
    );
    let content_id = doc.add_object(Stream::new(
        dictionary! {},
        content_text.into_bytes(),
    ));

    // Page dimensions match the embedded image (plus a small margin).
    let page_w: i64 = (img_w + 20) as i64;
    let page_h: i64 = (img_h + 20) as i64;

    // Pages tree
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), Object::from(page_w), Object::from(page_h)],
    });

    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::from(page_id)],
        "Count" => 1,
        "MediaBox" => vec![0.into(), 0.into(), Object::from(page_w), Object::from(page_h)],
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    // Catalog
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("save image-only secrets PDF");
    buf
}


/// Render the secret text into a PPM (P6) image using the 5×7 bitmap font.
///
/// **Note**: The 5x7 bitmap font is very crude — Tesseract struggles to
/// recognise characters even at large scales.  For reliable OCR tests use
/// **Note**: The 5x7 bitmap font is crude — Tesseract may struggle at low
/// scales. Scale=6 produces readable output for most secrets.
pub fn render_secrets_ppm() -> (Vec<u8>, u32, u32) {
    let text = concat!(
        "camille.demo@example.org\n",
        "06 12 34 56 78\n",
        "FR76 3000 6000 0112 3456 7890 189",
    );
    let scale = 6;
    let (rgb, w, h) = render_text_to_rgb(text, scale);

    let header = format!("P6\n{w} {h}\n255\n");
    let mut ppm = Vec::with_capacity(header.len() + rgb.len());
    ppm.extend_from_slice(header.as_bytes());
    ppm.extend_from_slice(&rgb);
    (ppm, w, h)
}
