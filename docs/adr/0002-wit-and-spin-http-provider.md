---
status: accepted
date: 2026-11-24
supersedes: []
references:
  - constitution.md
  - docs/adr/0001-image-metadata-architecture.md
---

# ADR 0002 — WIT Component Model Export und Fermyon Spin HTTP Provider

## Kontext

Mit ADR 0001 wurde `wasm_img-metadata` als reine Rust-CLI und Bibliothek realisiert, die EXIF-, IPTC- und XMP-Metadaten aus JPEG-, TIFF-, PNG- und WebP-Bilddateien extrahiert und als deterministisches JSON-Dokument ausgibt.

Ergänzend zum CLI-Modus sollen nun zwei weitere WebAssembly-Bereitstellungsmodi unterstützt werden:
1. **WASI Preview 2 Component Model (`wit`):** Andere WASI-Komponenten sollen Metadaten direkt per typisiertem Funktionsaufruf (`wit/metadata-provider.wit`) aus Dateipfaden oder Rohbytes extrahieren können.
2. **Fermyon Spin HTTP Service (`spin/spin.toml`, `src/provider`, `src/bin/wasm_img_metadata_spin.rs`):** Bereitstellung als serverloser Spin-HTTP-Endpunkt (`/v1/metadata/...`), über den Clients Bilddateien entweder als Raw-Image-Body (`image/jpeg`, `image/tiff`, `image/png`, `image/webp`, `application/octet-stream`) oder als JSON-Request (`{ "file": "...", "data_base64": "..." }`) senden und dasselbe deterministische JSON-Antwortschema (`SuccessResponse` / `ErrorResponse`) erhalten.

## Entscheidung

1. **WIT-Interface Definition (`wit/metadata-provider.wit`):**
   Es wird ein WIT World `metadata-provider` mit dem Interface `wasm-img:metadata/extractor@0.1.0` definiert:
   - `extract-from-path(image-path: string) -> result<string, string>`
   - `extract-from-bytes(file-label: string, image-bytes: list<u8>) -> result<string, string>`
2. **Schichtenarchitektur-Erweiterung (`src/provider`, `spin`):**
   - Die Verfassung (`constitution.md`) erweitert die Bereitstellungsschicht um `src/provider` (WIT Component Model Provider & Spin HTTP Handler), `src/bin/wasm_img_metadata_spin.rs` und `spin/spin.toml`.
   - `src/provider` nutzt ausschließlich die bestehende Kernlogik (`src/cli`, `src/container`, `src/exif`, `src/iptc`, `src/xmp`).
3. **Spin HTTP API Schema (`/v1/metadata/extract`):**
   - `POST /v1/metadata/extract`:
     - Akzeptiert entweder direkt rohe Bild-Bytes im HTTP-Request-Body (optionaler Header `X-File-Name` oder Query-Parameter `?file=...`, Standard `"upload"`) **oder** ein JSON-Payload `{ "file": "photo.jpg" }` (liest gemountete Datei im Spin-Sandbox-Dateisystem) und gibt das standardisierte JSON-Dokument (`{ "success": true, "file": "...", "metadata": { "exif": {}, "iptc": {}, "xmp": {} } }` bzw. `{ "success": false, "error": "..." }`) mit `Content-Type: application/json` zurück.
   - `GET /v1/metadata/health`: Gibt `{ "status": "ok", "service": "wasm-img-metadata-spin" }` zurück.

## Konsequenzen

- **Positiv:** Vollständige Kompatibilität mit Fermyon Spin (`spin up`, `spin build`) und `wasm32-wasip2` ohne C/C++-Abhängigkeiten.
- **Positiv:** Einheitlicher JSON-Vertrag über CLI, WIT-Component und Spin-HTTP-Service hinweg.
- **Negativ:** Zusätzliche Binär-Target-Definition (`wasm_img_metadata_spin`) in `Cargo.toml`.
