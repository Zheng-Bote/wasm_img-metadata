---
status: active
---

# Verfassung — wasm_img-metadata

Diese Verfassung enthält die systemweiten, dauerhaften Regeln des Projekts. Feature-Inhalte (User Stories, Tasklisten, Planung) gehören in PRD, Tech Spec und Plan. Jede Änderung einer Regel verweist auf ein ADR unter `docs/adr`.

## Architektur/Layering

1. Das System ist in klar getrennte Schichten unterteilt (`docs/adr/0001-image-metadata-architecture.md`):
   - `src/container`: Reines Demultiplexing der Bildcontainer-Formate (JPEG, TIFF, PNG, WebP) in Roh-Metadatenblöcke (EXIF, IPTC/8BIM, XMP) ohne Dateisystem-I/O.
   - `src/exif`: Reiner Parser für TIFF/EXIF-IFDs (IFD0, ExifIFD, GPSIFD, InteropIFD, SubIFDs, MakerNotes) inkl. Typkonvertierung, ISO-8601-Datumsnormalisierung und Erhalt unbekannter Tags.
   - `src/iptc`: Reiner Parser für IPTC-IIM-Datasets (inkl. Photoshop 8BIM Resource `0x0404`) mit Unterstützung für wiederholbare Felder (Arrays) und unbekannte Datasets.
   - `src/xmp`: Reiner Parser für XMP/RDF-XML-Pakete mit vollständigem Erhalt der Namespace-Präfixe (`dc:title`, `exif:*`, `xmp:*`, etc.), Arrays (`rdf:Bag`, `rdf:Seq`, `rdf:Alt`) und Typen.
   - `src/cli`: CLI-Argumentverarbeitung (`<image_path>`), Datei-I/O und deterministische JSON-Serialisierung (`serde`, `serde_json`) auf `stdout`.

## Security

1. Alle binären Offsets, Längenangaben, IFD-Zeigerzyklen und Zlib-komprimierten Blöcke (`zTXt`, `iTXt`) aus nicht vertrauenswürdigen Bilddateien müssen strikt gegen Pufferüberläufe, Endlosschleifen und Dekompressionsbomben geprüft werden (`docs/adr/0001-image-metadata-architecture.md`).
2. Das Programm darf bei fehlerhaften, abgeschnittenen oder unbekannten Metadaten niemals mit einem `panic!` abstürzen oder Metadaten stillschweigend verwerfen; ungültige Dateipfade oder nicht unterstützte Container liefern ein valides Fehler-JSON (`{"success": false, "error": "..."}`).

## Strikte Regeln

1. Jedes Akzeptanzkriterium (`AC-n`) im PRD muss durch mindestens einen automatisierten Test (`// <slug> AC-<n>: ...`) abgedeckt und verifiziert sein.
2. Der Code muss in reinem, stabilem Rust (`cargo test`, `cargo build`) ohne C/C++-Systemabhängigkeiten und warnungsfrei kompilieren (`docs/adr/0001-image-metadata-architecture.md`).
