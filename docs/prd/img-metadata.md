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
  - wit/**
  - spin/**
  - sdd.config.yaml
  - .github/ISSUE_TEMPLATE/feature_spec.md
---

# PRD — Image Metadata Extractor (EXIF, IPTC, XMP) (`img-metadata`)

## Problem

Entwickler, WASI-Komponenten und serverlose Microservices (Fermyon Spin) benötigen ein zuverlässiges, speichersicheres Werkzeug in Rust, das aus Bilddateien (JPEG, TIFF, PNG, WebP) sämtliche vorhandenen EXIF-, IPTC- und XMP-Metadaten verlustfrei extrahiert und als einheitliches, maschinenlesbares JSON-Dokument ausgibt — sowohl auf der Kommandozeile (`stdout`), über das WASI Component Model (`wit`) als auch als Fermyon Spin HTTP-Service (`/v1/metadata/extract`).

## Ziel

Bereitstellung einer vollständigen, mit stabilem Rust und Cargo ohne externe C-Abhängigkeiten kompilierbaren Anwendung (nativ und `wasm32-wasip2`), die Bilddateien über CLI (`<image_path>`), WIT-Interface und Spin-HTTP-Endpunkt verarbeitet, alle EXIF-, IPTC- und XMP-Metadaten extrahiert und ein strikt typisiertes JSON-Dokument liefert.

## Nutzer

- Backend-Dienste, Medien-Pipelines und CLI-Nutzer, die Bildmetadaten automatisiert als JSON weiterverarbeiten.
- WASI-Komponenten und Fermyon-Spin-Applikationen, die Metadaten-Extraktion serverlos über HTTP oder In-Process via WIT ausführen.

## Umfang

- CLI-Schnittstelle mit exakt einem Positionsargument `<image_path>`.
- Container-Unterstützung für JPEG, TIFF, PNG und WebP.
- Extraktion aller verfügbaren EXIF-, IPTC- und XMP-Metadaten.
- Erhalt originaler Feldnamen, Emission unbekannter Tags, Erhalt von Arrays und numerischen Werten, ISO-8601-Konvertierung von Datumswerten, leere Objekte `{}` für fehlende Sektionen.
- Strukturiertes JSON sowohl im Erfolgsfall (`"success": true`) als auch im Fehlerfall (`"success": false, "error": "..."`).
- WASI Component Model Interface (`wit/metadata-provider.wit`) und Provider (`src/provider/mod.rs`).
- Fermyon Spin HTTP Service (`spin/spin.toml`, `src/provider/http.rs`, `src/bin/wasm_img_metadata_spin.rs`).

## Nicht-Ziele

- Modifikation oder Zurückschreiben von Metadaten in Bilddateien.
- Dekodierung oder Transformation der eigentlichen Pixel-/Bilddaten.

## Akzeptanzkriterien

- **AC-1** Die Anwendung akzeptiert exakt ein Kommandozeilenargument `<image_path>`, lädt die Datei und gibt im Erfolgsfall ein valides JSON-Dokument mit `"success": true`, `"file": "<image_path>"` und `"metadata": { "exif": {...}, "iptc": {...}, "xmp": {...} }` auf `stdout` aus.
- **AC-2** Die Anwendung unterstützt die Bildformate JPEG, TIFF, PNG und WebP und extrahiert aus allen vier Formaten eingebettete EXIF-, IPTC- und XMP-Metadaten.
- **AC-3** Bekannte Felder behalten ihre originalen Feldnamen (z. B. `CameraModel` / `Model`, `Keywords`, `dc:title`), während unbekannte Tags in EXIF, IPTC und XMP niemals verworfen, sondern mit eindeutigen Schlüsseln ausgegeben werden.
- **AC-4** Datentypen werden originalgetreu abgebildet: Arrays bleiben JSON-Arrays, numerische Werte bleiben JSON-Zahlen, Datumsangaben werden nach Möglichkeit in ISO-8601-Strings konvertiert und fehlende Sektionen (`exif`, `iptc`, `xmp`) werden als leere JSON-Objekte `{}` ausgegeben.
- **AC-5** Bei fehlendem/ungültigem CLI-Argument, nicht existierender Datei oder ungültigem/nicht unterstütztem Bildformat gibt die Anwendung valides JSON der Form `{ "success": false, "error": "..." }` aus.
- **AC-6** Das WIT-Interface (`wit/metadata-provider.wit`) und `MetadataProvider` (`src/provider/mod.rs`) stellen `extract_from_path` und `extract_from_bytes` für WASI-Komponenten bereit und liefern dasselbe JSON-Dokument.
- **AC-7** Der Fermyon Spin HTTP Provider (`spin/spin.toml`, `src/provider/http.rs`, `src/bin/wasm_img_metadata_spin.rs`) verarbeitet `POST /v1/metadata/extract` (sowohl mit Raw-Image-Bytes als auch mit JSON-Pfad `{ "file": "..." }`) und `GET /v1/metadata/health` und liefert im Erfolgs- wie im Fehlerfall valides JSON.
