---
feature: img-metadata
status: approved
archive-after: 2027-01-01
---

# Plan — Image Metadata Extractor (EXIF, IPTC, XMP) (`img-metadata`)

Reihenfolge der Umsetzung. Jede Task nennt das Akzeptanzkriterium, auf das sie einzahlt.

## Tasks

- [x] AC-1 CLI-Schnittstelle, JSON-Ausgabemodell (`SuccessResponse`, `ErrorResponse`, `MetadataSections`) und Datei-Loader in `src/cli/mod.rs` und `src/main.rs` implementieren.
- [x] AC-2 Container-Demultiplexer für JPEG, TIFF, PNG und WebP in `src/container/mod.rs` implementieren.
- [x] AC-3 EXIF-, IPTC- und XMP-Parser mit vollständigem Erhalt originaler Feldnamen und unbekannter Tags in `src/exif/mod.rs`, `src/iptc/mod.rs` und `src/xmp/mod.rs` implementieren.
- [x] AC-4 Typkonvertierung (Zahlen, Arrays, ISO-8601-Daten) und Initialisierung leerer Sektionen (`{}`) sicherstellen.
- [x] AC-5 Fehlerbehandlung als garantierte JSON-Ausgabe (`{"success": false, "error": "..."}`) sowie automatisierte Tests für AC-1 bis AC-5 implementieren.
