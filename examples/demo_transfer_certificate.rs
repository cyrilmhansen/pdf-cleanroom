/// Synthetic demo: generate a fake bank transfer certificate PDF for testing.
///
/// This example generates a PDF with realistic-looking administrative content
/// containing fake secrets (email, phone, IBAN) so you can test scan, rebuild,
/// and preserve workflows without using real documents.
///
/// # Usage
///
/// ```sh
/// cargo run --example demo_transfer_certificate
/// ```
///
/// Generated PDF is written to `target/generated/demo-transfer-certificate.pdf`
/// (gitignored by `/target`).

use std::io::BufWriter;
use std::path::PathBuf;

use printpdf::*;

fn main() {
    let out_dir = PathBuf::from("target/generated");
    std::fs::create_dir_all(&out_dir).expect("create target/generated/");

    let output_path = out_dir.join("demo-transfer-certificate.pdf");

    let (doc, page, layer) = PdfDocument::new(
        "Virement SEPA — Attestation",
        Mm(210.0),
        Mm(297.0),
        "Content",
    );
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .expect("load built-in font");
    let layer = doc.get_page(page).get_layer(layer);

    // ── Header ──
    layer.use_text("ATTESTATION DE VIREMENT SEPA", 16.0, Mm(20.0), Mm(280.0), &font);
    layer.use_text("Banque Fictive SA — 75001 Paris", 10.0, Mm(20.0), Mm(272.0), &font);

    // ── Metadata ──
    layer.use_text("Date d'execution : 15/05/2026", 10.0, Mm(20.0), Mm(260.0), &font);
    layer.use_text("Reference : VIR-2026-08923", 10.0, Mm(20.0), Mm(252.0), &font);

    // ── Emetteur (Sender) ──
    layer.use_text("EMETTEUR", 12.0, Mm(20.0), Mm(240.0), &font);
    layer.use_text("Nom : Jean Demo", 10.0, Mm(20.0), Mm(232.0), &font);
    layer.use_text("Email : jean.demo@example.com", 10.0, Mm(20.0), Mm(224.0), &font);
    layer.use_text("Tel : 06 11 22 33 44", 10.0, Mm(20.0), Mm(216.0), &font);

    // ── Beneficiaire (Beneficiary) ──
    layer.use_text("BENEFICIAIRE", 12.0, Mm(20.0), Mm(204.0), &font);
    layer.use_text("Nom : SARL Exemple", 10.0, Mm(20.0), Mm(196.0), &font);
    layer.use_text("IBAN : FR76 3000 6000 0112 3456 7890 189", 10.0, Mm(20.0), Mm(188.0), &font);

    // ── Montant (Amount) ──
    layer.use_text("MONTANT", 12.0, Mm(20.0), Mm(176.0), &font);
    layer.use_text("Montant : 1 250,00 EUR", 10.0, Mm(20.0), Mm(168.0), &font);
    layer.use_text("Frais : 0,00 EUR", 10.0, Mm(20.0), Mm(160.0), &font);

    // ── Footer ──
    layer.use_text(
        "Ce document est une attestation automatique delivree par Banque Fictive SA.",
        8.0, Mm(20.0), Mm(30.0), &font,
    );

    let file = std::fs::File::create(&output_path).expect("create output file");
    doc.save(&mut BufWriter::new(file))
        .expect("save PDF");

    println!("Demo certificate generated: {}", output_path.display());
    println!();
    println!("Next steps:");
    println!("  # Scan for secrets");
    println!(
        "  pdf-cleanroom scan {}",
        output_path.display()
    );
    println!(
        "  # Rebuild with secrets masked (label mode)",
    );
    println!(
        "  pdf-cleanroom rebuild {} {} --mask label",
        output_path.display(),
        out_dir.join("demo-transfer-certificate-cleaned.pdf").display()
    );
}
