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
