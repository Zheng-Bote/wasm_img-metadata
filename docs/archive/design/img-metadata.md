---
feature: img-metadata
status: archived
archive-after: 2027-01-01
touches:
  - src/**
  - Cargo.toml
  - Cargo.lock
  - README.md
  - README_de.md
  - CHANGELOG.md
  - NOTICE
  - Example/**
  - tests/**
  - wit/**
  - spin/**
  - sdd.config.yaml
  - .gitignore
  - .github/ISSUE_TEMPLATE/feature_spec.md
release: v0.1.0
---

# Tech Spec — Image Metadata Extractor (EXIF, IPTC, XMP) (`img-metadata`)

Setzt das PRD `docs/prd/img-metadata.md` um. Die Verfassung hat Vorrang; diese Tech Spec darf sie präzisieren, nie lockern.

## Architektur

Die Anwendung gliedert sich in sechs Module gemäß `constitution.md`, `docs/adr/0001-image-metadata-architecture.md` und `docs/adr/0002-wit-and-spin-http-provider.md`:

1. `src/container/mod.rs`: Erkennt anhand der Magic-Bytes das Containerformat (JPEG `FF D8`, TIFF `II*\0` / `MM\0*`, PNG `\x89PNG\r\n\x1a\n`, WebP `RIFF....WEBP`) und extrahiert alle Roh-Segmente für EXIF, IPTC und XMP.
2. `src/exif/mod.rs`: Parst TIFF-Header und IFD-Ketten (IFD0, IFD1, ExifIFD, GPSIFD, InteropIFD, SubIFDs), löst alle TIFF-Typen in `serde_json::Value` auf, normalisiert EXIF-Zeitstempel nach ISO-8601 und extrahiert eingebettete IPTC- (`0x83BB`), Photoshop-IRB- (`0x8649`) und XMP-Tags (`0x02BC`).
3. `src/iptc/mod.rs`: Parst Photoshop IRB (`8BIM` `0x0404`) sowie rohe IPTC-IIM-Datasets (`0x1C` Tag-Marker). Wiederholbare Datasets (`Keywords` 2:25) werden immer als JSON-Arrays abgebildet, Datumswerte (`YYYYMMDD`) nach ISO-8601 (`YYYY-MM-DD`) konvertiert.
4. `src/xmp/mod.rs`: Parst XMP-XML/RDF-Pakete vollständig in Schlüssel-Wert-Paare unter Beibehaltung der XML-Namespace-Präfixe (`dc:title`, `dc:subject`, `xmp:CreateDate`, etc.), löst `rdf:Seq`, `rdf:Bag` und `rdf:Alt` auf und konvertiert Zahlen/Booleans/ISO-8601-Daten.
5. `src/cli/mod.rs` & `src/main.rs`: Nimmt `<image_path>` entgegen, orchestriert Datei-I/O und Demuxing/Parsing und gibt das Ergebnis als `serde_json`-Dokument auf `stdout` aus.
6. `src/provider/mod.rs`, `src/provider/http.rs`, `src/bin/wasm_img_metadata_spin.rs`, `wit/metadata-provider.wit`, `spin/spin.toml`:
   - Exportiert das WASI Component Model Interface (`wasm-img:metadata/extractor@0.1.0`).
   - Implementiert den Fermyon Spin HTTP Service (`/v1/metadata/extract` und `/v1/metadata/health`), kompilierbar für `wasm32-wasip2`.

## Schnittstellen

### 1. CLI-Aufruf

```bash
cargo run -- <image_path>
```

### 2. WIT Component Interface (`wit/metadata-provider.wit`)

```wit
package wasm-img:metadata@0.1.0;

interface extractor {
    extract-from-path: func(image-path: string) -> result<string, string>;
    extract-from-bytes: func(file-label: string, image-bytes: list<u8>) -> result<string, string>;
}

world metadata-provider {
    export extractor;
}
```

### 3. Fermyon Spin HTTP Service (`spin/spin.toml`)

- `POST /v1/metadata/extract`:
  - **Modus A (JSON-Request auf gemountete Datei):** `Content-Type: application/json` mit Body `{"file": "Example/sample.jpg"}`.
  - **Modus B (Direkter Binär-Upload):** Beliebige Bild-Bytes im Request-Body (`image/jpeg`, `image/png`, `image/tiff`, `image/webp`, `application/octet-stream`), Dateiname über Header `X-File-Name: photo.jpg` oder Query `?file=photo.jpg`.
- `GET /v1/metadata/health`:
  - Antwortet mit `200 OK` und `{"status":"ok","service":"wasm-img-metadata-spin"}`.

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
| AC-6 | `src/provider/mod.rs`, `wit/metadata-provider.wit` | `test_ac6_wit_metadata_provider` |
| AC-7 | `src/provider/http.rs`, `src/bin/wasm_img_metadata_spin.rs`, `spin/spin.toml` | `test_ac7_spin_http_provider` |

## Sicherheit

- Alle Slice-Zugriffe beim Binär-Parsing nutzen sichere Bounds-Checks (`get(...)`), sodass korrupte Offsets in EXIF-IFDs, JPEG-Markern, PNG-Chunks oder WebP-RIFF-Containern niemals zu einem Panic führen.
- IFD-Offset-Tracking verhindert Endlosschleifen durch zyklische IFD-Pointer.
- Zlib-Dekompression in PNG-Chunks ist größenbeschränkt (16 MiB Maximalgrenze gegen Zip-Bomben).
- Spin-HTTP-Endpunkte geben niemals ungeprüfte Stacktraces aus, sondern stets das strukturierte JSON-Fehlerschema (`{"success": false, "error": "..."}`).

## Offene Punkte

- Keine offenen Punkte.
