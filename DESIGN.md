# pdf-cleanroom — Design (MVP)

## Objectif
Extraire le texte visible d'un PDF, détecter les secrets (email, téléphone FR, IBAN), et reconstruire un PDF neuf sans ces secrets. Le rebuilt ne copie jamais la structure source — il crée un document indépendant avec printpdf.

## Bibliothèques
- **lopdf** (retenu) — extraction texte, manipulation bas niveau des objets PDF, patching de métadata/annotations/ formulaires.
- **printpdf** (retenu) — reconstruction PDF neuf (document vierge, texte visible uniquement).
- **regex** — détection par motifs.
- **serde_json + SHA-256** — rapport + hash des secrets.
- pdfium-render (écarté — C++ lourd), krilla (écarté — instable), redactor/MuPDF (écarté — réservé preserve futur).

## Choix retenus
- Extraction via lopdf extract_text().
- Reconstruction via printpdf (perte de mise en page, polices, images — acceptable pour MVP).
- Détection regex sur le texte extrait.
- Rapport JSON avec hash SHA-256 des secrets.
- Masquage par blocs noirs, libellés ([EMAIL]), ou hash (#...).

## Choix écartés
- Superposition rectangles noirs sur PDF source (interdit).
- Copie structure source (interdit — neuf garantit aucune fuite).
- Mode preview (interdit — pas de rendu bitmappé).
- Preserve (non implémenté — n'accepterait que le strict nécessaire le jour venu).

## Stratégies de sortie
L'utilisateur choisit une stratégie via `--strategy` :

- `text-only` (défaut) : reconstruit un PDF neuf à partir du seul texte visible extrait. Les images, annotations, formulaires et métadonnées source ne sont pas copiés. C'est le comportement MVP.
- `flatten-visible` : destiné à préserver le contenu visible utile (images, mise en page) à l'avenir. Actuellement partiel — les images ne sont pas encore sanitisées. Un avertissement est émis à l'utilisation.
- `preserve` (non implémenté) : refus explicite — ne sera jamais simulé par superposition de rectangles.

Quelle que soit la stratégie, le rebuilt crée toujours un document printpdf indépendant ; il ne copie jamais la structure source.

## Architecture
```
main → cli → safety → detect::scan → report → rebuild::clean_pdf [→ rapport JSON]
```
## Limites connues
- lopdf ignore annotations, formulaires, flux non standard, métadonnées XMP.
- printpdf perd mise en page, polices personnalisées, images.
- Pas de garantie d'extraction complète (text caché, fragmenté, chevauché).
- Pas d'OCR (les secrets dans les images ne sont pas détectés).
- PDF chiffrés → rejet explicite avec erreur (lopdf ne déchiffre pas).

## Tests — Suite hostile (v1)
Tous les PDFs de test sont générés dynamiquement (pas de fixtures binaires).

Cas couverts :
1. Texte visible standard → détection + rebuilt sans secrets.
2. Métadonnées classiques (Title, Author…) → absentes du rebuilt.
3. Métadonnées XMP → non copiées.
4. Fichiers embarqués → supprimés.
5. Annotations → supprimées.
6. Champs de formulaire (AcroForm) → supprimés.
7. Texte caché (blanc sur blanc, hors limites, minuscule) → comportement documenté.
8. Texte recouvert par une forme opaque → extractible (documenté).
9. Secrets fragmentés (split Tj/Td) → détectés si l'extracteur les fusionne.
10. PDF image-only → aucune détection (pas d'OCR).
11. PDF chiffré → erreur explicite.
12. --dry-run → ne crée pas de fichier de sortie.
13. --unsafe-show-secrets → exact_value absent par défaut, présent avec le flag.
14. Régression octets bruts → rebuilt ne contient aucun secret source.

Chaque rebuilt est vérifié : texte extrait exempt de secrets ; octets bruts exempts de secrets ; objets source absents.

## Critères de succès
- cargo test vert (62 tests unité + intégration).
- scan détecte emails/tél/IBAN.
- rebuilt produit PDF sans secrets.
- --dry-run ne modifie rien, --unsafe-show-secrets contrôle l'affichage.
- --strategy text-only (défaut) / flatten-visible (expérimental, avertit).

## OCR et images
- **OCR** : non implémenté par défaut. Architecture d'accueil définie dans `src/ocr.rs` avec trait `OcrEngine` et implémentation `NoopOcrEngine` (toujours indisponible). Le jour où un backend OCR est ajouté, il reste optionnel et limité à la détection.
- **Images dans le source** : `text-only` ne copie jamais les images. `flatten-visible` ne les préserve pas encore (partiel). Les secrets dans les images ne sont pas détectés sans OCR.
- **Source dans le rapport** : chaque occurrence peut indiquer sa source (`pdf_text`, `metadata`, `annotation`, `ocr`, etc.) via le champ optionnel `source` dans `SecretOccurrence`. Par défaut, la source est `"pdf_text"` pour les détections issues du texte extrait.
- **Tests** : `tests/integration_ocr.rs` (3 tests) vérifie que les secrets dans les images ne sont pas détectés sans OCR, que le rebuilt text-only supprime les images, et que flatten-visible documente sa limitation.

