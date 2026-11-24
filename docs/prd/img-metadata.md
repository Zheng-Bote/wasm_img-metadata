---
feature: img-metadata
status: approved
archive-after: 2027-01-01
touches:
  - src/**
  - Cargo.toml
  - Cargo.lock
  - README.md
  - tests/**
---

# PRD — Image Metadata Extractor (EXIF, IPTC, XMP) (`img-metadata`)

## Problem

Entwickler und automatisierte Pipelines benötigen ein zuverlässiges, speichersicheres CLI-Werkzeug in Rust, das aus Bilddateien (JPEG, TIFF, PNG, WebP) sämtliche vorhandenen EXIF-, IPTC- und XMP-Metadaten verlustfrei extrahiert und als einheitliches, maschinenlesbares JSON-Dokument auf `stdout` ausgibt — ohne unbekannte Tags stillschweigend zu verwerfen oder bei Fehlern nicht-JSON-Ausgaben zu erzeugen.

## Ziel

Bereitstellung einer vollständigen, mit stabilem Rust und Cargo ohne externe C-Abhängigkeiten kompilierbaren Anwendung, die einen einzelnen Bildpfad (`<image_path>`) entgegennimmt, alle EXIF-, IPTC- und XMP-Metadaten extrahiert und ein strikt typisiertes JSON-Dokument auf `stdout` ausgibt.

## Nutzer

- Backend-Dienste, Medien-Pipelines und CLI-Nutzer, die Bildmetadaten automatisiert als JSON weiterverarbeiten.
- WASI-/Rust-Anwendungen, die eine reine Rust-Bibliothek und CLI ohne native C++-Abhängigkeiten benötigen.

## Umfang

- CLI-Schnittstelle mit exakt einem Positionsargument `<image_path>`.
- Container-Unterstützung für JPEG, TIFF, PNG und WebP.
- Extraktion aller verfügbaren EXIF-, IPTC- und XMP-Metadaten.
- Erhalt originaler Feldnamen, Emission unbekannter Tags, Erhalt von Arrays und numerischen Werten, ISO-8601-Konvertierung von Datumswerten, leere Objekte `{}` für fehlende Sektionen.
- Strukturiertes JSON sowohl im Erfolgsfall (`"success": true`) als auch im Fehlerfall (`"success": false, "error": "..."`).

## Nicht-Ziele

- Modifikation oder Zurückschreiben von Metadaten in Bilddateien.
- Dekodierung oder Transformation der eigentlichen Pixel-/Bilddaten.

## Akzeptanzkriterien

- **AC-1** Die Anwendung akzeptiert exakt ein Kommandozeilenargument `<image_path>`, lädt die Datei und gibt im Erfolgsfall ein valides JSON-Dokument mit `"success": true`, `"file": "<image_path>"` und `"metadata": { "exif": {...}, "iptc": {...}, "xmp": {...} }` auf `stdout` aus.
- **AC-2** Die Anwendung unterstützt die Bildformate JPEG, TIFF, PNG und WebP und extrahiert aus allen vier Formaten eingebettete EXIF-, IPTC- und XMP-Metadaten.
- **AC-3** Bekannte Felder behalten ihre originalen Feldnamen (z. B. `CameraModel` / `Model`, `Keywords`, `dc:title`), während unbekannte Tags in EXIF, IPTC und XMP niemals verworfen, sondern mit eindeutigen Schlüsseln ausgegeben werden.
- **AC-4** Datentypen werden originalgetreu abgebildet: Arrays bleiben JSON-Arrays, numerische Werte bleiben JSON-Zahlen, Datumsangaben werden nach Möglichkeit in ISO-8601-Strings konvertiert und fehlende Sektionen (`exif`, `iptc`, `xmp`) werden als leere JSON-Objekte `{}` ausgegeben.
- **AC-5** Bei fehlendem/ungültigem CLI-Argument, nicht existierender Datei oder ungültigem/nicht unterstütztem Bildformat gibt die Anwendung valides JSON der Form `{ "success": false, "error": "..." }` aus.
