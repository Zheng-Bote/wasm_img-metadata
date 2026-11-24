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

# Tech Spec — Image Metadata Extractor (EXIF, IPTC, XMP) (`img-metadata`)

Setzt das PRD `docs/prd/img-metadata.md` um. Die Verfassung hat Vorrang; diese Tech Spec darf sie präzisieren, nie lockern.

## Architektur

Die Anwendung gliedert sich in fünf Schichten gemäß `constitution.md` und `docs/adr/0001-image-metadata-architecture.md`:

1. `src/container/mod.rs`: Erkennt anhand der Magic-Bytes das Containerformat (JPEG `FF D8`, TIFF `II*\0` / `MM\0*`, PNG `\x89PNG\r\n\x1a\n`, WebP `RIFF....WEBP`) und extrahiert alle Roh-Segmente für EXIF, IPTC und XMP.
2. `src/exif/mod.rs`: Parst TIFF-Header und IFD-Ketten (IFD0, IFD1, ExifIFD, GPSIFD, InteropIFD, SubIFDs), löst Typen (BYTE, ASCII, SHORT, LONG, RATIONAL, SBYTE, UNDEFINED, SSHORT, SLONG, SRATIONAL, FLOAT, DOUBLE) in `serde_json::Value` auf, normalisiert EXIF-Zeitstempel (`YYYY:MM:DD HH:MM:SS` plus optionale `SubSecTime*` / `OffsetTime*`) nach ISO-8601 und extrahiert zudem in TIFF-IFDs eingebettete IPTC- (`0x83BB`) und XMP-Tags (`0x02BC`).
3. `src/iptc/mod.rs`: Parst Photoshop IRB (`8BIM` Ressourcen, insbesondere `0x0404` für IPTC-IIM) sowie rohe IPTC-IIM-Datasets (`0x1C` Tag-Marker, Record, Dataset, Länge). Wiederholbare Datasets (wie `Keywords` 2:25) werden immer als JSON-Arrays abgebildet, Datumswerte (`YYYYMMDD`) nach ISO-8601 (`YYYY-MM-DD`) konvertiert.
4. `src/xmp/mod.rs`: Parst XMP-XML/RDF-Pakete vollständig in Schlüssel-Wert-Paare unter Beibehaltung der XML-Namespace-Präfixe (`dc:title`, `dc:subject`, `xmp:CreateDate`, etc.), löst `rdf:Seq`, `rdf:Bag` und `rdf:Alt` auf und konvertiert reine Zahlen/Booleans in entsprechende JSON-Typen.
5. `src/cli/mod.rs` & `src/main.rs`: Nimmt `<image_path>` entgegen, orchestriert Datei-I/O und Demuxing/Parsing und gibt das Ergebnis als `serde_json`-Dokument auf `stdout` aus.

## Schnittstellen

CLI-Aufruf:

```bash
cargo run -- <image_path>
```

Erfolgsausgabe (`stdout`):

```json
{
  "success": true,
  "file": "photo.jpg",
  "metadata": {
    "exif": {},
    "iptc": {},
    "xmp": {}
  }
}
```

Fehlerausgabe (`stdout`):

```json
{
  "success": false,
  "error": "..."
}
```

## Datenmodell

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetadataSections {
    pub exif: serde_json::Map<String, serde_json::Value>,
    pub iptc: serde_json::Map<String, serde_json::Value>,
    pub xmp: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuccessResponse {
    pub success: bool,
    pub file: String,
    pub metadata: MetadataSections,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}
```

## Umsetzung der Akzeptanzkriterien

| AC | Umsetzung | Test |
|---|---|---|
| AC-1 | `src/cli/mod.rs`, `src/main.rs` | `test_ac1_cli_and_json_structure` |
| AC-2 | `src/container/mod.rs` | `test_ac2_formats_jpeg_tiff_png_webp` |
| AC-3 | `src/exif/mod.rs`, `src/iptc/mod.rs`, `src/xmp/mod.rs` | `test_ac3_preserve_names_and_unknown_tags` |
| AC-4 | `src/exif/mod.rs`, `src/iptc/mod.rs`, `src/xmp/mod.rs` | `test_ac4_types_arrays_numbers_dates_empty_sections` |
| AC-5 | `src/cli/mod.rs` | `test_ac5_error_handling_valid_json` |

## Sicherheit

- Alle Slice-Zugriffe beim Binär-Parsing nutzen sichere Bounds-Checks (`get(...)`), sodass korrupte Offsets in EXIF-IFDs, JPEG-Markern, PNG-Chunks oder WebP-RIFF-Containern niemals zu einem Panic führen.
- IFD-Offset-Tracking verhindert Endlosschleifen durch zyklische IFD-Pointer.
- Zlib-Dekompression in PNG-Chunks ist größenbeschränkt (Maximalgrenze gegen Zip-Bomben).

## Offene Punkte

- Keine offenen Punkte.
