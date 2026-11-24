---
name: Feature Specification (Spec Kit)
about: Propose a new feature, container format support, or metadata parser rule for wasm_img-metadata.
title: "Feature: "
labels: "feature, spec-kit, wasi"
assignees: ""
---

## Feature Intent

_Describe what the feature does and why it is needed. Keep it concise. Reference current architecture (WASI CLI, Container demuxing, EXIF/TIFF parser, IPTC parser, XMP parser, JSON Output) where relevant._

## Requirements (EARS Syntax)

_Use EARS patterns: `THE SYSTEM SHALL …` (ubiquitous), `WHEN <trigger>, THE SYSTEM SHALL …` (event), `WHILE <state>, …` (state), `WHERE <feature/option>, …` (optional), `IF <condition>, THEN THE SYSTEM SHALL …` (unwanted behaviour)._

1. **WHEN** _trigger_, **THE SYSTEM SHALL** _response_.
2. **IF** _condition_, **THEN THE SYSTEM SHALL** _response_.
3. **THE SYSTEM SHALL** _response_.

## Scope

_Which layers and files are affected? Delete lines that do not apply._

- **Container Demuxing (JPEG / TIFF / PNG / WebP):** `src/container/`
- **EXIF / TIFF Binary Tag Parser:** `src/exif/`
- **IPTC IIM & Photoshop 8BIM Parser:** `src/iptc/`
- **XMP Packet / RDF XML Parser:** `src/xmp/`
- **CLI & JSON Output Formatting:** `src/cli/`, `src/main.rs`, `src/lib.rs`
- **Documentation & Examples:** `README.md`, `README_de.md`

### Open Decisions

_Questions that must be answered before or during implementation (delete if none)._

## SpecDD Architecture Alignment (Drift Control)

Please confirm that this feature respects the constraints defined in `constitution.md`:

- [ ] **Layering:** The 4-layer architecture is maintained (`src/container`, `src/exif`, `src/iptc`, `src/xmp`, `src/cli`). Container and metadata parsers do not depend on CLI or I/O layers.
- [ ] **Lossless Metadata Preservation:** Original field names are preserved whenever possible; unknown tags are never silently discarded (`0xNNNN` / dataset IDs / namespace keys are emitted).
- [ ] **Type Fidelity:** Arrays remain JSON arrays, numeric values remain numeric, dates are normalized to ISO-8601 strings where possible, and missing sections (`exif`, `iptc`, `xmp`) are emitted as empty JSON objects `{}`.
- [ ] **Deterministic JSON Contract:** Every execution (including CLI errors, file read errors, or corrupted container headers) returns valid JSON (`{"success": true, ...}` or `{"success": false, "error": "..."}`) to stdout.
- [ ] **Memory Safety & DoS Resilience:** Untrusted binary headers, offsets, IFD pointers, and compressed chunks are bounds-checked without out-of-bounds panics or infinite loops.
- [ ] **Pure Rust & WASI Compatibility:** Compiles cleanly on stable Rust using pure Rust dependencies (`serde`, `serde_json`, `flate2`) without warnings.
- [ ] **Standards:** License, `constitution.md`, SDD artifacts, and documentation are maintained.

## Acceptance Criteria

- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Tests added/updated: `cargo test` and SDD gates pass (`sdd lint`, `sdd gate pre-merge`).
- [ ] Documentation updated: `README.md` is up to date.
