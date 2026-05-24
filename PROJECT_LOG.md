# Project Log — pdf-cleanroom

## 2026-05-24 — Initialisation projet

- Création du projet Rust `pdf-cleanroom` avec `cargo init --name pdf-cleanroom`.
- Rédaction DESIGN.md : choix lopdf (extraction) + printpdf (reconstruction), refus explicite des rectangles noirs.
- Création de ce journal de bord.
- Cargo.toml configuré avec clap, regex, serde, sha2, lopdf, printpdf.

## 2026-05-24 — Structure et modules

- Création de src/main.rs, cli.rs, detect.rs, mask.rs, report.rs, pdf_extract.rs, rebuild.rs, safety.rs.
- Implémentation CLI : sous-commandes scan, rebuild, preserve ; options --dry-run, --unsafe-show-secrets, --mask.
- Module detect : emails, téléphones français, IBAN via regex.
- Module mask : black-block, label, same-width, thin-air, hash.
- Module report : structure JSON avec masquage et SHA-256.
- Module safety : vérifications et avertissements avant exécution.

## 2026-05-24 — Extraction PDF et reconstruction

- Module pdf_extract : extraction de texte par page via `lopdf::Document::extract_text()`.
- Module rebuild : reconstruction d'un nouveau PDF via `printpdf` avec texte nettoyé.
- Première compilation réussie après ajustements des dépendances.
- Bug notable : `lopdf::Document::extract_text()` ne documente pas la gestion des polices non encodées → extraction partielle possible.
- Décision : accepté comme limite MVP documentée.

## 2026-05-24 — Correction regex téléphone

- Bug : la regex téléphone français `0[1-9](?:...){4}` capturait des sous-chaînes d'IBAN (ex: `0532013000` dans `DE89370400440532013000`).
- Correction : ajout de `\b` avant le `0` initial pour exiger une frontière de mot.
- La variante internationale `+33...` n'utilise pas `\b` car `+` est déjà non-mot.

## 2026-05-24 — Tests et finalisation

- Tests unitaires pour detect (emails, téléphones, IBAN).
- Tests unitaires pour mask (tous les modes).
- Tests d'intégration scan et rebuild avec mini PDF généré en mémoire.
- README rédigé avec avertissements de sécurité.
- Décision : les tests d'intégration utilisent printpdf pour générer un mini PDF texte → pas de dépendance à un fichier externe.
- `cargo test` : 33 tests passent.

## 2026-05-24 — DESIGN.md condensé

- DESIGN.md réduit de 48 à 15 lignes pour respecter la limite de 30 lignes.
- Contenu conservé : objectif MVP, choix bibliothèques, limites sécurité, architecture, critères réussite.
- Aucune information perdue.

## 2026-05-24 — Suite de tests hostile / edge-case PDF
- Ajout de `tests/support/pdf_fixtures.rs` : génération dynamique de PDF (lopdf/printpdf) pour métadonnées, XMP, fichiers embarqués, annotations, formulaires, textes cachés, secrets fragmentés, PDF image-only et chiffrés.
- Ajout de `tests/integration_hostile_pdf.rs` (15 tests) : couvre les 14 cas hostiles : texte visible, métadonnées, XMP, pièces jointes, annotations, formulaires, textes non visibles, textes recouverts, secrets fragmentés, image-only, PDF chiffré, dry-run, unsafe-show-secrets, régression octets bruts.
- Ajout de `tests/integration_sanitization.rs` (8 tests) : vérifie les modes de masquage, dry-run, unsafe-show-secrets, et garanties de non-fuite des secrets dans le rebuilt.
- Correction : utilisation de répertoires temporaires uniques (pid + compteur atomique) pour éviter les races conditions entre tests parallèles.
- `cargo test` : 56 tests passent (29 unitaires, 27 intégration).

## 2026-05-24 — Stratégie de sortie --strategy
- Ajout de l'option `--strategy` (global) : `text-only` (défaut) et `flatten-visible` (expérimental).
- Ajout de l'énum `cli::Strategy` avec les variantes `TextOnly` et `FlattenVisible`.
- `flatten-visible` émet un avertissement explicite sur l'absence de sanitisation d'images.
- `preserve` reste non implémenté et ne sera jamais simulé par superposition de rectangles.
- DESIGN.md mis à jour avec la section « Stratégies de sortie ».
- `cargo test` : 56 tests passent (aucune régression).

## 2026-05-24 — Tests métadonnées, annotations, fichiers embarqués
- Vérification des trois tests hostiles PDF générés dynamiquement : métadonnées classiques, annotations, fichiers embarqués.
- Chaque PDF contient les trois types de secrets (email, téléphone FR, IBAN).
- `assert_no_secret_bytes` vérifie que le rebuilt ne contient aucun secret dans ses octets bruts (nom de fichier inclus pour les pièces jointes).
- Documentation explicite des limitations de détection : lopdf n'extrait pas les métadonnées, annotations, ni fichiers embarqués.
- `cargo test` : 56 tests passent (aucune régression).

## 2026-05-24 — Architecture OCR et tests images
- Création de `src/ocr.rs` : trait `OcrEngine`, structure `OcrFinding`, erreur `OcrError`, implémentation `NoopOcrEngine` (toujours indisponible).
- Ajout de `pub mod ocr` dans `lib.rs`.
- Ajout du champ optionnel `source` dans `SecretOccurrence` (rapport JSON) pour distinguer `pdf_text`, `metadata`, `annotation`, `form`, `attachment`, `ocr`.
- Ajout de `add_occurrence_with_source()` dans `Report`.
- Ajout de `tests/support/pdf_fixtures.rs` : `image_embedded_only_pdf()` et `image_embedded_with_text_pdf()` (vraies images bitmap embarquées).
- Ajout de `tests/integration_ocr.rs` (3 tests) : image sans OCR non détectée, text-only supprime les images, flatten-visible documenté.
- DESIGN.md mis à jour avec section « OCR et images », README.md avec note flatten-visible.
- `cargo test` : 62 tests passent (56 existants + 3 OCR unité + 3 OCR intégration).

## 2026-05-24 — Nettoyage documentation
- Phrase maladroite sur le redaction pixel-level corrigée dans README.md.
- DESIGN.md réduit de 86 à 79 lignes (suppression blancs, architecture et vérifications condensées).
- `cargo test` : 62 tests passent (aucune régression).

## 2026-05-24 — Preserve backends, rebuild quality, demo, CLI
- Investigation documentée dans PRESERVE_BACKENDS.md : rôles des outils (rendu, inspection qpdf, rédaction destructive redactor, extraction lopdf, OCR Tesseract). qpdf --json=2 validé sur 15 PDFs de test.
- preserve reste non implémenté ; message d'erreur amélioré avec référence au backend nécessaire.
- Trait `PdfTextExtractor` + implémentation `LopdfExtractor` ajoutés dans `pdf_extract.rs`.
- Reconstructions text-only améliorées : espacement des lignes 6pt→14pt, marges, habillage de mots (>85 car.), gestion UTF-8 multi-octets.
- Fixtures ajoutées : `accented_text_pdf()`, `admin_document_pdf()`.
- 3 nouveaux tests integration_rebuild : accents, document admin, ordre des labels (65 tests, 0 échec).
- README.md : exemples rebuild enrichis, aide CLI du rebuilder enrichie.
- Nouvel exemple `examples/demo_transfer_certificate.rs` génère une fausse attestation de virement.
- `cargo test` : 65 tests passent (62 existants + 3 qualité rebuild).

## 2026-05-24 — Benchmark design & visual metrics skeleton
- `BENCHMARKS.md` créé : métriques (PSNR, SSIM, MAE, phash), comparaison redaction-aware, format de sortie `target/pdf-cleanroom-bench/`, plan d'implémentation en 5 phases.
- Renderers documentés : pdftoppm, mutool draw, Ghostscript, pdfium-render (tous optionnels, invoqués via `std::process::Command`).
- `tests/support/visual_metrics.rs` ajouté : `VisualBenchmarkResult`, `PageVisualMetrics`, `RenderedPage`, `MaskRegion`, `PixelMetrics`, `SanitizationChecks`, `BenchConfig`, `BenchSummary` — définitions uniquement, pas de rendu.
- `tests/support/mod.rs` mis à jour avec `pub mod visual_metrics`.
- Aucune dépendance ajoutée, aucun rendu implémenté.

## 2026-05-24 — Optional Tesseract OCR smoke test
- `tests/integration_ocr_smoke.rs` : test complet du pipeline OCR (PDF → rendu PNG → tesseract → Détecteur → Rapport).
- Gating via `PDF_CLEANROOM_OCR_TESTS=1` ; normal `cargo test` ne dépend pas de Tesseract.
- Détection externe de renderer (pdftoppm > mutool) et de tesseract (PATH).
- PDF généré via `visible_text_pdf()`, rendu en PNG sous `target/pdf-cleanroom-ocr/`.
- tesseract invoqué avec `-l fra+eng` pour une meilleure reconnaissance des IBAN/téléphones français.
- Assertion : au moins 1 secret détecté (la qualité OCR varie, pas tous les secrets plantés sont exigés).
- Intégration Report avec `source: "ocr"` exercée.
- Texte OCR affiché sur stderr pour diagnostic en cas d'échec.
- README.md : section « Optional OCR smoke tests » ajoutée (prérequis, limitations).
- Aucune dépendance Rust ajoutée, aucun rendu implémenté.

### Correction — image-only PDF fix et test PPM/Pillow
- Le fixture `image_only_secrets_pdf()` utilise désormais `render_secrets_ppm()` pour éviter la duplication du texte secret.
- Le test principal OCR utilise Python+Pillow pour générer une image texte nette (police DejaVuSans, 18pt), évitant la police bitmaps 5×7 trop grossière pour Tesseract.
- Fallback si Pillow n'est pas installé : skip avec message clair (code retour 2).
- `render_secrets_ppm()` conserve l'approche bitmap 5×7 (scale=4) pour le PDF embed, avec documentation explicite des limites.
- Le test image-only PDF reste SKIP avec TODO (problème de rendu DeviceRGB non résolu).
- Testé avec Pillow 11.x + Tesseract 5.5.2 : 1 test passe (pipelines Pillow + visible-text PDF).
- `cargo test` : 66 tests passent (0 régression).
- Testé avec tesseract 5.5.2 + pdftoppm 26.05.0 : 1 test passe (pipeline complet).
- `cargo test` : 66 tests passent (0 régression).
- `cargo test` : 65 tests passent (aucune régression).

### 2026-05-24 — Image-only PDF rendering fix
- **Root cause**: `/Im0 Do` maps the image to the unit square [0,1]×[0,1] in
  user space, then transforms by the CTM.  With identity CTM (no `cm`
  operator), ANY image renders at 1×1 point — invisible at typical DPI.
- **Fix**: Added `{width} 0 0 {height} 20 20 cm` before `/Im0 Do` in the
  content stream, scaling the unit square to the image's pixel dimensions.
- `render_secrets_ppm()` scale augmenté de 4 → 6 (meilleure lisibilité).
- `image_only_secrets_pdf()` refactored for reuse via `render_secrets_ppm()`.
- Test image-only PDF activé : vérification structurelle (taille PNG > 10 Ko
  prouvant que l'image XObject s'est rendue correctement).
- Les polices bitmaps 5×7 restent trop grossières pour Tesseract — la
  détection OCR réelle utilise le test Pillow (police DejaVuSans, 18pt).
- `cargo test` : 66 tests passent (0 régression).

### 2026-05-24 — flatten-raster strategy
- Nouveau module `src/flatten.rs` : détection de renderer (pdftoppm/mutool/gs),
  rendu de page→PPM, construction PDF image-only via lopdf.
- Variant `FlattenRaster` ajouté à l'enum `Strategy`.
- `--strategy flatten-raster` dans le CLI : rend chaque page en image,
  reconstruit un PDF sans texte sélectionnable.
- Avertissement clair : les secrets visibles dans l'image NE sont PAS masqués.
- Échec explicite si aucun renderer externe n'est trouvé.
- `tests/integration_flatten.rs` : test optionnel (PDF_CLEANROOM_FLATTEN_TESTS=1).
- Norme `cargo test` : 68 tests passent (0 régression, 1 ignoré visuel).
- README.md et DESIGN.md mis à jour.

### 2026-05-24 — Pixel mask regions foundation
- Ajout de `flatten::MaskRegion { page, x, y, width, height }` en coordonnées pixels de l'image rendue (origine haut-gauche).
- Ajout de `apply_pixel_masks()` : rectangles noirs, clipping aux dimensions image, modification directe du buffer RGB avant embedding.
- Ajout de `flatten_pdf_with_masks()` pour plomberie interne/tests sans CLI publique de rédaction.
- Tests unitaires pixels + test d'intégration optionnel flatten-raster : PDF image-only, pas de texte extractible, stream image décodé avec pixels masqués vérifiés.

### 2026-05-24 — Build/version metadata
- Ajout de `build.rs` : export du commit court, branche et état dirty via variables d'environnement de compilation, avec fallback `unknown`.
- Ajout de `pdf-cleanroom --version` et `pdf-cleanroom version` : version crate, commit, branche, dirty, profil debug/release.
- README enrichi avec exemple diagnostic et exemple `--strategy flatten-raster`.
