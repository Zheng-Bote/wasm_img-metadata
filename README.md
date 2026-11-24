# wasm_img-metadata

Pure-Rust image metadata extraction CLI and library developed with Spec-Driven Development (`sdd`).
Accepts a single `<image_path>` argument, extracts **all** available **EXIF**, **IPTC**, and **XMP** metadata from **JPEG**, **TIFF**, **PNG**, and **WebP** images, and returns a single deterministic JSON document to `stdout`.

## Features & Rules

- **Pure Stable Rust:** Built with `serde`, `serde_json`, and `flate2` (for compressed PNG `zTXt`/`iTXt` chunks). Compiles out of the box without C/C++ bindings.
- **Supported Formats:**
  - **JPEG** (`APP1` EXIF, `APP1` XMP & Extended XMP, `APP13` Photoshop IRB `8BIM` / IPTC-IIM, `COM`)
  - **TIFF** (Little-Endian `II*\0` & Big-Endian `MM\0*`, IFD0/IFD1, ExifIFD, GPSIFD, InteropIFD, SubIFDs, embedded IPTC `0x83BB`, Photoshop IRB `0x8649`, embedded XMP `0x02BC`)
  - **PNG** (`IHDR`, `eXIf`, `iTXt` including `XML:com.adobe.xmp`, `tEXt`, `zTXt`, `tIME`, `pHYs`, Raw profiles)
  - **WebP** (RIFF `WEBP` container with `EXIF`, `XMP `, `IPTC`/`PHOT`, `VP8X`)
- **Lossless Metadata Rules:**
  - Original field names are preserved (`CameraModel`, `Model`, `Make`, `Keywords`, `dc:title`, etc.).
  - Unknown tags are never discarded (`Tag_0xNNNN`, `GPSTag_0xNNNN`, `IPTC_<record>_<dataset>`, custom XML namespaces).
  - Arrays remain JSON arrays, numeric values remain JSON numbers, dates are normalized to ISO-8601 strings where possible, and missing sections are emitted as empty JSON objects `{}`.
  - Errors always emit valid JSON (`{"success": false, "error": "..."}`) to `stdout`.

## Build Instructions

```bash
# Build debug binary
cargo build

# Build optimized release binary
cargo build --release

# Run test suite
cargo test

# Run SDD verification gates
sdd lint
sdd gate pre-merge
```

## Example Execution

```bash
cargo run -- photo.jpg
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
