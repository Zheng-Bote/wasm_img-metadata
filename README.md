# wasm_img-metadata

Pure-Rust image metadata extraction CLI, WASI Component (`wit`), and **Fermyon Spin** HTTP Service developed with Spec-Driven Development (`sdd`).
Accepts an `<image_path>` (via CLI or Spin HTTP / WIT), extracts **all** available **EXIF**, **IPTC**, and **XMP** metadata from **JPEG**, **TIFF**, **PNG**, and **WebP** images, and returns a single deterministic JSON document.

## Features & Rules

- **Pure Stable Rust & WASI Preview 2 (`wasm32-wasip2`):** Built with `serde`, `serde_json`, and `flate2` (for compressed PNG `zTXt`/`iTXt` chunks). Compiles out of the box for native and `wasm32-wasip2` targets without C/C++ bindings.
- **Three Execution Modes:**
  1. **CLI Binary (`wasm_img-metadata`):** `cargo run -- <image_path>`
  2. **Fermyon Spin HTTP Microservice (`wasm_img_metadata_spin` / `spin/spin.toml`):** Serverless HTTP endpoint at `/v1/metadata/extract` and `/v1/metadata/health`.
  3. **WASI Preview 2 Component Model (`wit/metadata-provider.wit`):** Typed interface `wasm-img:metadata/extractor@0.1.0` (`extract-from-path`, `extract-from-bytes`).
- **Supported Formats:**
  - **JPEG** (`APP1` EXIF, `APP1` XMP & Extended XMP, `APP13` Photoshop IRB `8BIM` / IPTC-IIM, `COM`)
  - **TIFF** (Little-Endian `II*\0` & Big-Endian `MM\0*`, IFD0/IFD1, ExifIFD, GPSIFD, InteropIFD, SubIFDs, embedded IPTC `0x83BB`, Photoshop IRB `0x8649`, embedded XMP `0x02BC`)
  - **PNG** (`IHDR`, `eXIf`, `iTXt` including `XML:com.adobe.xmp`, `tEXt`, `zTXt`, `tIME`, `pHYs`, Raw profiles)
  - **WebP** (RIFF `WEBP` container with `EXIF`, `XMP `, `IPTC`/`PHOT`, `VP8X`)
- **Lossless Metadata Rules:**
  - Original field names are preserved (`CameraModel`, `Model`, `Make`, `Keywords`, `dc:title`, etc.).
  - Unknown tags are never discarded (`Tag_0xNNNN`, `GPSTag_0xNNNN`, `IPTC_<record>_<dataset>`, custom XML namespaces).
  - Arrays remain JSON arrays, numeric values remain JSON numbers, dates are normalized to ISO-8601 strings where possible, and missing sections are emitted as empty JSON objects `{}`.
  - Errors always emit valid JSON (`{"success": false, "error": "..."}`).

## Build Instructions

```bash
# 1. Build native CLI & Spin binaries
cargo build --release

# 2. Build WASI Preview 2 WebAssembly component for Fermyon Spin
cargo build --bin wasm_img_metadata_spin --target wasm32-wasip2 --release

# 3. Run test suite (AC-1 .. AC-7)
cargo test

# 4. Run SDD verification gates
sdd lint
sdd gate pre-merge
```

## Example Execution

### 1. CLI Execution

```bash
cargo run -- photo.jpg
```

### 2. Fermyon Spin HTTP Execution

Start the Spin service using `spin/spin.toml`:

```bash
spin build --from spin/spin.toml
spin up --from spin/spin.toml
```

Query via raw binary upload or JSON path reference:

```bash
# Option A: Upload raw image bytes directly
curl -X POST http://127.0.0.1:3000/v1/metadata/extract \
  -H "Content-Type: image/jpeg" \
  -H "X-File-Name: photo.jpg" \
  --data-binary @photo.jpg

# Option B: Reference a mounted file inside the Spin sandbox
curl -X POST http://127.0.0.1:3000/v1/metadata/extract \
  -H "Content-Type: application/json" \
  -d '{"file": "Example/photo.jpg"}'

# Option C: Health check
curl http://127.0.0.1:3000/v1/metadata/health
```

### Example Success Output

```json
{
  "success": true,
  "file": "photo.jpg",
  "metadata": {
    "exif": {
      "Make": "Canon",
      "Model": "Canon EOS R5",
      "CameraModel": "Canon EOS R5",
      "Orientation": 1,
      "XResolution": 300,
      "FNumber": 2.8,
      "Tag_0xC123": 987654,
      "ISOSpeedRatings": 400,
      "DateTimeOriginal": "2026-05-14T10:30:45",
      "GPSLatitudeRef": "N",
      "GPSLatitude": [
        52,
        31,
        12.5
      ],
      "GPSTag_0x0099": 7
    },
    "iptc": {
      "ObjectName": "Alpine Sunrise",
      "Keywords": [
        "rust",
        "metadata",
        "exif"
      ],
      "DateCreated": "2026-05-14",
      "IPTC_2_201": "CustomIptcValue"
    },
    "xmp": {
      "xmp:Rating": 5,
      "xmp:CreateDate": "2026-05-14T10:30:45",
      "custom:UnknownProp": "preserved-value",
      "dc:title": "Sunset Over Mountains",
      "dc:subject": [
        "landscape",
        "alps"
      ]
    }
  }
}
```

### Example Error Output

```json
{
  "success": false,
  "error": "Failed to read file 'missing.jpg': No such file or directory (os error 2)"
}
```
