---
status: accepted
date: 2026-11-24
supersedes: []
references:
  - constitution.md
---

# ADR 0001 — Pure Rust Image Metadata Extraction Architecture

## Kontext

Bilddateien in den Formaten JPEG, TIFF, PNG und WebP betten Metadaten in drei primären Standards ein: EXIF (TIFF-basierte IFDs), IPTC-IIM (häufig in Photoshop IRB 8BIM `0x0404` oder direkt als Record-2-Datasets) und XMP (RDF/XML-Pakete). Externe C++-Bibliotheken wie `libexiv2` erschweren die Portabilität (insbesondere für WebAssembly/WASI) und bergen Speichersicherheitsrisiken beim Parsen nicht vertrauenswürdiger Binärdaten. Gleichzeitig fordert die Spezifikation den vollständigen Erhalt aller bekannten und unbekannten Tags, Original-Feldnamen, numerischer Typen, Arrays und ISO-8601-Datumsformate sowie eine garantierte JSON-Ausgabe über `serde` und `serde_json`.

## Entscheidung

1. **Pure-Rust-Schichtenarchitektur:** Die Extraktion erfolgt vollständig in stabilem, reinem Rust ohne native C/C++-Bindungen (`libexiv2` wird nicht benötigt). Für JSON-Serialisierung werden `serde` und `serde_json` verwendet; für komprimierte PNG-Metadatenblöcke (`zTXt`, komprimiertes `iTXt`) wird `flate2` eingesetzt.
2. **Container-Demultiplexing (`src/container`):**
   - **JPEG:** Scannt APP1 (`Exif\0\0`, XMP `http://ns.adobe.com/xap/1.0/\0` und Extended XMP), APP13 (`Photoshop 3.0\0` 8BIM IRB Blöcke sowie direkte IPTC-IIM-Records).
   - **TIFF:** Parst Little-Endian (`II*\0`) und Big-Endian (`MM\0*`) Header und extrahiert neben allen Standard- und Sub-IFDs auch eingebettete IPTC-Tags (`0x83BB` / 33723) und XMP-Tags (`0x02BC` / 700).
   - **PNG:** Parst Chunks (`eXIf`, `iTXt` inkl. `XML:com.adobe.xmp`, `tEXt`, `zTXt`, `tIME`, `pHYs`, Raw-Profile-Blöcke für EXIF/IPTC/XMP).
   - **WebP:** Parst RIFF/WEBP-Chunks (`EXIF`, `XMP `, sowie animierte/erweiterte Chunks `VP8X` und `ICCP`/unbekannte Metadaten-Chunks).
3. **Verlustfreie Tag-Abbildung (`src/exif`, `src/iptc`, `src/xmp`):**
   - **EXIF:** Bekannte Tags werden auf ihre kanonischen Originalnamen (`CameraModel` / `Model`, `Make`, `DateTimeOriginal`, `GPSLatitude`, etc.) abgebildet; unbekannte Tags werden als `Tag_0xNNNN` (bzw. mit IFD-Kontext bei Kollisionen) ausgegeben. Rationale Werte und Zahlen bleiben JSON-Zahlen (oder Arrays von Zahlen), Datumsstrings (`YYYY:MM:DD HH:MM:SS`) werden nach ISO-8601 (`YYYY-MM-DDTHH:MM:SS`) konvertiert.
   - **IPTC:** Bekannte Record-2- und andere IPTC-Datasets erhalten ihre Standardnamen (`Keywords`, `ObjectName`, `Caption-Abstract`, `By-line`, `DateCreated`, etc.); unbekannte Datasets werden als `IPTC_Record_Dataset` (`IPTC_2_123`) erhalten. Wiederholbare Felder wie `Keywords` werden stets als JSON-Arrays geführt.
   - **XMP:** Parst `rdf:Description`-Attribute, Kind-Elemente, `rdf:Bag`, `rdf:Seq` und `rdf:Alt` (inkl. `xml:lang`-Qualifizierer oder Standardwert) unter Beibehaltung der Original-Präfixnamen (`dc:title`, `dc:subject`, `xmp:CreateDate`, etc.) und Typkonvertierung für Zahlen/Booleans/ISO-8601-Daten.
4. **Deterministischer JSON-Vertrag (`src/cli`):** Jeder Programmaufruf gibt exakt ein valides JSON-Dokument auf `stdout` aus (`success: true` mit `file` und `metadata: { exif: {}, iptc: {}, xmp: {} }` oder `success: false` mit `error`).

## Konsequenzen

- **Positiv:** 100 % speichersicheres, portables Rust (kompiliert ohne C-Toolchain sofort mit `cargo build` und ist WASI-kompatibel).
- **Positiv:** Kein stillschweigender Verlust unbekannter oder herstellerspezifischer Tags in EXIF, IPTC oder XMP.
- **Negativ:** Umfangreiche Tag-Wörterbücher für EXIF und IPTC müssen direkt im Rust-Code gepflegt werden.
