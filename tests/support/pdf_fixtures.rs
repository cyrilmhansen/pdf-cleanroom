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
