# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-24

### Added
- **Core Pure-Rust Image Metadata Extractor (`wasm_img-metadata`):**
  - Container demultiplexing (`src/container/`) for **JPEG** (`APP1` EXIF, `APP1` XMP & Extended XMP, `APP13` Photoshop IRB `8BIM` / IPTC-IIM, `COM`), **TIFF** (Little-Endian `II*\0` & Big-Endian `MM\0*`), **PNG** (`IHDR`, `eXIf`, `iTXt`, `tEXt`, `zTXt`, `tIME`, `pHYs`, Raw profiles), and **WebP** (RIFF `WEBP` with `EXIF`, `XMP `, `IPTC`/`PHOT`, `VP8X`).
  - Complete **EXIF / TIFF** parser (`src/exif/`) supporting IFD0, IFD1, ExifIFD, GPSIFD, InteropIFD, SubIFDs, embedded IPTC (`0x83BB`), Photoshop IRB (`0x8649`), and embedded XMP (`0x02BC`).
  - Complete **IPTC-IIM** parser (`src/iptc/`) supporting repeatable dataset arrays (`Keywords`, `By-line`, etc.), ISO-8601 date normalization, and preservation of unknown datasets (`IPTC_<record>_<dataset>`).
  - Complete **XMP / RDF-XML** parser (`src/xmp/`) preserving original namespace prefixes (`dc:title`, `dc:subject`, `exif:*`, `photoshop:*`, `Iptc4xmpCore:*`, etc.), `rdf:Bag`, `rdf:Seq`, and `rdf:Alt` collections, numeric types, and ISO-8601 timestamps.
  - Deterministic JSON output contract (`src/cli/`) via `serde` and `serde_json` on `stdout` (`{"success": true, "file": "...", "metadata": {"exif": {...}, "iptc": {...}, "xmp": {...}}}` and `{"success": false, "error": "..."}`).
- **WASI Preview 2 Component Model (`wit`):**
  - Exported WIT world `metadata-provider` and interface `wasm-img:metadata/extractor@0.1.0` in `wit/metadata-provider.wit` (`extract-from-path`, `extract-from-bytes`).
  - Provider implementation in `src/provider/mod.rs`.
- **Fermyon Spin HTTP Service (`wasi:http` / Wagi):**
  - Spin manifest `spin/spin.toml` configuring route `/v1/metadata/...` and mounting `Example/`.
  - Spin HTTP handler (`src/provider/http.rs`) and binary (`src/bin/wasm_img_metadata_spin.rs`) supporting `POST /v1/metadata/extract` (raw binary upload and JSON path reference `{"file": "..."}`) and `GET /v1/metadata/health`.
- **Documentation, Examples & SDD Governance:**
  - Real-world sample photograph `Example/2011-06-04_131209.jpg` (containing rich EXIF, GPS, IPTC, and XMP metadata) and step-by-step guide `Example/README.md`.
  - Comprehensive English (`README.md`) and German (`README_de.md`) documentation.
  - Adapted GitHub issue template (`.github/ISSUE_TEMPLATE/feature_spec.md`).
  - Apache-2.0 `NOTICE` file.
  - Full Spec-Driven Development (`sdd`) specification suite (`constitution.md`, ADRs 0001–0002, PRD, Tech Spec, Plan, and acceptance test suite `AC-1`–`AC-7`).
