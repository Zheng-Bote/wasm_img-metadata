use std::fs;
use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde_json::{json, Value};
use wasm_img_metadata::cli::{extract_metadata_from_bytes, run_cli_with_args};

/// Helper to build a synthetic Little-Endian TIFF/EXIF binary containing:
/// - Make ("Canon")
/// - Model / CameraModel ("Canon EOS R5")
/// - Orientation (1 - numeric SHORT)
/// - DateTimeOriginal ("2026:05:14 10:30:45" -> ISO-8601 "2026-05-14T10:30:45")
/// - FNumber (RATIONAL 28/10 -> 2.8)
/// - ISOSpeedRatings (SHORT 400)
/// - Unknown tag 0xC123 (LONG 987654)
/// - GPSIFD with GPSLatitude ([52, 31, 12.5]) and unknown GPS tag 0x0099 (SHORT 7)
fn build_sample_tiff_exif(include_embedded_iptc_xmp: bool) -> Vec<u8> {
    let mut buf = Vec::new();
    // TIFF header: "II*\0" + offset to IFD0 (8)
    buf.extend_from_slice(b"II*\0");
    buf.extend_from_slice(&8u32.to_le_bytes());

    // We will construct IFD0 at offset 8
    // Entries in IFD0:
    // 1. 0x010F Make (ASCII)
    // 2. 0x0110 Model (ASCII)
    // 3. 0x0112 Orientation (SHORT = 1)
    // 4. 0x011A XResolution (RATIONAL = 300/1 -> 300)
    // 5. 0x829D FNumber (RATIONAL = 28/10 -> 2.8)
    // 6. 0x8769 ExifOffset (LONG -> ExifIFD)
    // 7. 0x8825 GPSInfo (LONG -> GPSIFD)
    // 8. 0xC123 Unknown Tag (LONG = 987654)
    // Optional:
    // 9. 0x02BC Embedded XMP
    // 10. 0x83BB Embedded IPTC
    let base_entries = if include_embedded_iptc_xmp { 10u16 } else { 8u16 };
    let ifd0_size = 2 + (base_entries as usize) * 12 + 4;
    let data_area_start = 8 + ifd0_size;

    let mut data_area: Vec<u8> = Vec::new();
    let push_data = |bytes: &[u8], data_area: &mut Vec<u8>| -> u32 {
        let offset = (data_area_start + data_area.len()) as u32;
        data_area.extend_from_slice(bytes);
        if data_area.len() % 2 != 0 {
            data_area.push(0);
        }
        offset
    };

    let make_bytes = b"Canon\0";
    let make_off = push_data(make_bytes, &mut data_area);

    let model_bytes = b"Canon EOS R5\0";
    let model_off = push_data(model_bytes, &mut data_area);

    let mut xres_bytes = Vec::new();
    xres_bytes.extend_from_slice(&300u32.to_le_bytes());
    xres_bytes.extend_from_slice(&1u32.to_le_bytes());
    let xres_off = push_data(&xres_bytes, &mut data_area);

    let mut fnum_bytes = Vec::new();
    fnum_bytes.extend_from_slice(&28u32.to_le_bytes());
    fnum_bytes.extend_from_slice(&10u32.to_le_bytes());
    let fnum_off = push_data(&fnum_bytes, &mut data_area);

    // Prepare ExifIFD data:
    // 1. 0x8827 ISOSpeedRatings (SHORT = 400)
    // 2. 0x9003 DateTimeOriginal (ASCII "2026:05:14 10:30:45\0")
    let dto_bytes = b"2026:05:14 10:30:45\0";
    let dto_off = push_data(dto_bytes, &mut data_area);

    let exif_ifd_off = (data_area_start + data_area.len()) as u32;
    data_area.extend_from_slice(&2u16.to_le_bytes());
    // Entry 1: ISOSpeedRatings (0x8827), SHORT (3), count 1, value 400
    data_area.extend_from_slice(&0x8827u16.to_le_bytes());
    data_area.extend_from_slice(&3u16.to_le_bytes());
    data_area.extend_from_slice(&1u32.to_le_bytes());
    data_area.extend_from_slice(&400u16.to_le_bytes());
    data_area.extend_from_slice(&0u16.to_le_bytes());
    // Entry 2: DateTimeOriginal (0x9003), ASCII (2), count 20, offset dto_off
    data_area.extend_from_slice(&0x9003u16.to_le_bytes());
    data_area.extend_from_slice(&2u16.to_le_bytes());
    data_area.extend_from_slice(&(dto_bytes.len() as u32).to_le_bytes());
    data_area.extend_from_slice(&dto_off.to_le_bytes());
    // Next IFD = 0
    data_area.extend_from_slice(&0u32.to_le_bytes());

    // Prepare GPSIFD data:
    // 1. 0x0001 GPSLatitudeRef (ASCII "N\0")
    // 2. 0x0002 GPSLatitude (RATIONAL count 3: 52/1, 31/1, 125/10)
    // 3. 0x0099 Unknown GPS tag (SHORT = 7)
    let mut gps_lat_bytes = Vec::new();
    for (n, d) in [(52u32, 1u32), (31u32, 1u32), (125u32, 10u32)] {
        gps_lat_bytes.extend_from_slice(&n.to_le_bytes());
        gps_lat_bytes.extend_from_slice(&d.to_le_bytes());
    }
    let gps_lat_off = push_data(&gps_lat_bytes, &mut data_area);

    let gps_ifd_off = (data_area_start + data_area.len()) as u32;
    data_area.extend_from_slice(&3u16.to_le_bytes());
    // GPSLatitudeRef
    data_area.extend_from_slice(&0x0001u16.to_le_bytes());
    data_area.extend_from_slice(&2u16.to_le_bytes());
    data_area.extend_from_slice(&2u32.to_le_bytes());
    data_area.extend_from_slice(b"N\0\0\0");
    // GPSLatitude
    data_area.extend_from_slice(&0x0002u16.to_le_bytes());
    data_area.extend_from_slice(&5u16.to_le_bytes());
    data_area.extend_from_slice(&3u32.to_le_bytes());
    data_area.extend_from_slice(&gps_lat_off.to_le_bytes());
    // Unknown GPS tag 0x0099
    data_area.extend_from_slice(&0x0099u16.to_le_bytes());
    data_area.extend_from_slice(&3u16.to_le_bytes());
    data_area.extend_from_slice(&1u32.to_le_bytes());
    data_area.extend_from_slice(&7u16.to_le_bytes());
    data_area.extend_from_slice(&0u16.to_le_bytes());
    // Next IFD = 0
    data_area.extend_from_slice(&0u32.to_le_bytes());

    let (xmp_len, xmp_off, iptc_len, iptc_off) = if include_embedded_iptc_xmp {
        let xmp = build_sample_xmp().into_bytes();
        let x_len = xmp.len() as u32;
        let x_off = push_data(&xmp, &mut data_area);

        let iptc = build_sample_iptc_iim();
        let i_len = iptc.len() as u32;
        let i_off = push_data(&iptc, &mut data_area);
        (x_len, x_off, i_len, i_off)
    } else {
        (0, 0, 0, 0)
    };

    // Write IFD0
    buf.extend_from_slice(&base_entries.to_le_bytes());

    let write_entry = |buf: &mut Vec<u8>, tag: u16, ty: u16, count: u32, val_or_off: [u8; 4]| {
        buf.extend_from_slice(&tag.to_le_bytes());
        buf.extend_from_slice(&ty.to_le_bytes());
        buf.extend_from_slice(&count.to_le_bytes());
        buf.extend_from_slice(&val_or_off);
    };

    write_entry(
        &mut buf,
        0x010F,
        2,
        make_bytes.len() as u32,
        make_off.to_le_bytes(),
    );
    write_entry(
        &mut buf,
        0x0110,
        2,
        model_bytes.len() as u32,
        model_off.to_le_bytes(),
    );
    write_entry(&mut buf, 0x0112, 3, 1, [1, 0, 0, 0]);
    write_entry(&mut buf, 0x011A, 5, 1, xres_off.to_le_bytes());
    if include_embedded_iptc_xmp {
        write_entry(&mut buf, 0x02BC, 1, xmp_len, xmp_off.to_le_bytes());
    }
    write_entry(&mut buf, 0x829D, 5, 1, fnum_off.to_le_bytes());
    if include_embedded_iptc_xmp {
        write_entry(&mut buf, 0x83BB, 7, iptc_len, iptc_off.to_le_bytes());
    }
    write_entry(&mut buf, 0x8769, 4, 1, exif_ifd_off.to_le_bytes());
    write_entry(&mut buf, 0x8825, 4, 1, gps_ifd_off.to_le_bytes());
    write_entry(&mut buf, 0xC123, 4, 1, 987_654u32.to_le_bytes());

    // Next IFD offset = 0
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&data_area);

    buf
}

fn build_sample_iptc_iim() -> Vec<u8> {
    let mut iptc = Vec::new();
    let mut push_dataset = |record: u8, dataset: u8, payload: &[u8]| {
        iptc.push(0x1C);
        iptc.push(record);
        iptc.push(dataset);
        iptc.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        iptc.extend_from_slice(payload);
    };

    // ObjectName (2:5)
    push_dataset(2, 5, b"Alpine Sunrise");
    // Keywords (2:25) - repeatable!
    push_dataset(2, 25, b"rust");
    push_dataset(2, 25, b"metadata");
    push_dataset(2, 25, b"exif");
    // DateCreated (2:55) - YYYYMMDD -> ISO-8601 YYYY-MM-DD
    push_dataset(2, 55, b"20260514");
    // Unknown IPTC dataset (2:201)
    push_dataset(2, 201, b"CustomIptcValue");

    iptc
}

fn build_photoshop_irb_with_iptc(iptc: &[u8]) -> Vec<u8> {
    let mut irb = Vec::new();
    irb.extend_from_slice(b"8BIM");
    irb.extend_from_slice(&0x0404u16.to_be_bytes());
    // Empty Pascal string (2 bytes: 0x00, 0x00)
    irb.extend_from_slice(&[0x00, 0x00]);
    irb.extend_from_slice(&(iptc.len() as u32).to_be_bytes());
    irb.extend_from_slice(iptc);
    if iptc.len() % 2 != 0 {
        irb.push(0);
    }
    irb
}

fn build_sample_xmp() -> String {
    r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
        xmlns:dc="http://purl.org/dc/elements/1.1/"
        xmlns:xmp="http://ns.adobe.com/xap/1.0/"
        xmlns:custom="http://example.com/ns/"
        xmp:Rating="5"
        xmp:CreateDate="2026:05:14 10:30:45"
        custom:UnknownProp="preserved-value">
      <dc:title>
        <rdf:Alt>
          <rdf:li xml:lang="x-default">Sunset Over Mountains</rdf:li>
        </rdf:Alt>
      </dc:title>
      <dc:subject>
        <rdf:Bag>
          <rdf:li>landscape</rdf:li>
          <rdf:li>alps</rdf:li>
        </rdf:Bag>
      </dc:subject>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#
        .to_string()
}

fn build_sample_jpeg() -> Vec<u8> {
    let mut jpg = vec![0xFF, 0xD8]; // SOI

    // APP1 EXIF
    let tiff = build_sample_tiff_exif(false);
    let mut exif_seg = Vec::new();
    exif_seg.extend_from_slice(b"Exif\0\0");
    exif_seg.extend_from_slice(&tiff);
    jpg.extend_from_slice(&[0xFF, 0xE1]);
    jpg.extend_from_slice(&((exif_seg.len() + 2) as u16).to_be_bytes());
    jpg.extend_from_slice(&exif_seg);

    // APP1 XMP
    let xmp = build_sample_xmp();
    let mut xmp_seg = Vec::new();
    xmp_seg.extend_from_slice(b"http://ns.adobe.com/xap/1.0/\0");
    xmp_seg.extend_from_slice(xmp.as_bytes());
    jpg.extend_from_slice(&[0xFF, 0xE1]);
    jpg.extend_from_slice(&((xmp_seg.len() + 2) as u16).to_be_bytes());
    jpg.extend_from_slice(&xmp_seg);

    // APP13 Photoshop IRB with IPTC
    let iptc = build_sample_iptc_iim();
    let irb = build_photoshop_irb_with_iptc(&iptc);
    let mut app13_seg = Vec::new();
    app13_seg.extend_from_slice(b"Photoshop 3.0\0");
    app13_seg.extend_from_slice(&irb);
    jpg.extend_from_slice(&[0xFF, 0xED]);
    jpg.extend_from_slice(&((app13_seg.len() + 2) as u16).to_be_bytes());
    jpg.extend_from_slice(&app13_seg);

    // EOI
    jpg.extend_from_slice(&[0xFF, 0xD9]);
    jpg
}

fn build_sample_png() -> Vec<u8> {
    let mut png = Vec::from(&b"\x89PNG\r\n\x1a\n"[..]);
    let write_chunk = |png: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]| {
        png.extend_from_slice(&(data.len() as u32).to_be_bytes());
        png.extend_from_slice(chunk_type);
        png.extend_from_slice(data);
        png.extend_from_slice(&0u32.to_be_bytes()); // dummy CRC
    };

    // IHDR (13 bytes)
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&1920u32.to_be_bytes());
    ihdr.extend_from_slice(&1080u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    write_chunk(&mut png, b"IHDR", &ihdr);

    // eXIf chunk
    let tiff = build_sample_tiff_exif(false);
    write_chunk(&mut png, b"eXIf", &tiff);

    // iTXt chunk for XMP ("XML:com.adobe.xmp\0" + compression_flag(0) + method(0) + lang("\0") + trans("\0") + xml)
    let mut itxt = Vec::new();
    itxt.extend_from_slice(b"XML:com.adobe.xmp\0");
    itxt.push(0); // uncompressed
    itxt.push(0);
    itxt.push(0); // empty lang
    itxt.push(0); // empty translated keyword
    itxt.extend_from_slice(build_sample_xmp().as_bytes());
    write_chunk(&mut png, b"iTXt", &itxt);

    // zTXt chunk for Raw profile type iptc
    let iptc = build_sample_iptc_iim();
    let hex_iptc: String = iptc.iter().map(|b| format!("{b:02x}")).collect();
    let raw_profile = format!("\niptc\n{}\n{}\n", iptc.len(), hex_iptc);
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(raw_profile.as_bytes()).unwrap();
    let compressed_profile = encoder.finish().unwrap();

    let mut ztxt = Vec::new();
    ztxt.extend_from_slice(b"Raw profile type iptc\0");
    ztxt.push(0); // compression method deflate
    ztxt.extend_from_slice(&compressed_profile);
    write_chunk(&mut png, b"zTXt", &ztxt);

    write_chunk(&mut png, b"IEND", &[]);
    png
}

fn build_sample_webp() -> Vec<u8> {
    let mut chunks = Vec::new();
    let write_riff_chunk = |buf: &mut Vec<u8>, fourcc: &[u8; 4], data: &[u8]| {
        buf.extend_from_slice(fourcc);
        buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
        buf.extend_from_slice(data);
        if data.len() % 2 != 0 {
            buf.push(0);
        }
    };

    let tiff = build_sample_tiff_exif(false);
    write_riff_chunk(&mut chunks, b"EXIF", &tiff);

    let xmp = build_sample_xmp();
    write_riff_chunk(&mut chunks, b"XMP ", xmp.as_bytes());

    let iptc = build_sample_iptc_iim();
    write_riff_chunk(&mut chunks, b"IPTC", &iptc);

    let mut webp = Vec::new();
    webp.extend_from_slice(b"RIFF");
    webp.extend_from_slice(&((4 + chunks.len()) as u32).to_le_bytes());
    webp.extend_from_slice(b"WEBP");
    webp.extend_from_slice(&chunks);
    webp
}

// img-metadata AC-1: CLI accepts a single <image_path> argument and outputs valid JSON with success, file, and metadata sections
#[test]
fn test_ac1_cli_and_json_structure() {
    let tmp_dir = std::env::temp_dir();
    let file_path = tmp_dir.join("test_ac1_photo.jpg");
    fs::write(&file_path, build_sample_jpeg()).expect("write sample jpeg");

    let path_str = file_path.to_string_lossy().to_string();
    let (json_out, code) = run_cli_with_args(["wasm_img-metadata", &path_str]);
    let _ = fs::remove_file(&file_path);

    assert_eq!(code, 0);
    let parsed: Value = serde_json::from_str(&json_out).expect("valid JSON on stdout");
    assert_eq!(parsed["success"], json!(true));
    assert_eq!(parsed["file"], json!(path_str));
    assert!(parsed["metadata"]["exif"].is_object());
    assert!(parsed["metadata"]["iptc"].is_object());
    assert!(parsed["metadata"]["xmp"].is_object());
    assert_eq!(parsed["metadata"]["exif"]["CameraModel"], json!("Canon EOS R5"));
    assert_eq!(parsed["metadata"]["iptc"]["Keywords"], json!(["rust", "metadata", "exif"]));
    assert_eq!(parsed["metadata"]["xmp"]["dc:title"], json!("Sunset Over Mountains"));
}

// img-metadata AC-2: Supports JPEG, TIFF, PNG, and WebP containers and extracts EXIF, IPTC, and XMP from all four
#[test]
fn test_ac2_formats_jpeg_tiff_png_webp() {
    let containers: Vec<(&str, Vec<u8>)> = vec![
        ("sample.jpg", build_sample_jpeg()),
        ("sample.tiff", build_sample_tiff_exif(true)),
        ("sample.png", build_sample_png()),
        ("sample.webp", build_sample_webp()),
    ];

    for (name, bytes) in containers {
        let res = extract_metadata_from_bytes(name, &bytes)
            .unwrap_or_else(|e| panic!("Failed on {name}: {}", e.error));
        assert!(res.success, "Expected success for {name}");
        assert_eq!(res.file, name);
        assert_eq!(
            res.metadata.exif.get("CameraModel"),
            Some(&json!("Canon EOS R5")),
            "EXIF CameraModel mismatch in {name}"
        );
        assert_eq!(
            res.metadata.iptc.get("Keywords"),
            Some(&json!(["rust", "metadata", "exif"])),
            "IPTC Keywords mismatch in {name}"
        );
        assert_eq!(
            res.metadata.xmp.get("dc:title"),
            Some(&json!("Sunset Over Mountains")),
            "XMP dc:title mismatch in {name}"
        );
    }
}

// img-metadata AC-3: Preserves original field names and emits unknown tags in EXIF, IPTC, and XMP without discarding metadata
#[test]
fn test_ac3_preserve_names_and_unknown_tags() {
    let jpg = build_sample_jpeg();
    let res = extract_metadata_from_bytes("photo.jpg", &jpg).expect("extract jpeg");

    // Original names preserved
    assert_eq!(res.metadata.exif.get("Make"), Some(&json!("Canon")));
    assert_eq!(res.metadata.exif.get("Model"), Some(&json!("Canon EOS R5")));
    assert_eq!(res.metadata.exif.get("CameraModel"), Some(&json!("Canon EOS R5")));
    assert_eq!(res.metadata.iptc.get("ObjectName"), Some(&json!("Alpine Sunrise")));
    assert_eq!(res.metadata.xmp.get("dc:title"), Some(&json!("Sunset Over Mountains")));

    // Unknown tags emitted
    assert_eq!(res.metadata.exif.get("Tag_0xC123"), Some(&json!(987654)));
    assert_eq!(res.metadata.exif.get("GPSTag_0x0099"), Some(&json!(7)));
    assert_eq!(res.metadata.iptc.get("IPTC_2_201"), Some(&json!("CustomIptcValue")));
    assert_eq!(
        res.metadata.xmp.get("custom:UnknownProp"),
        Some(&json!("preserved-value"))
    );
}

// img-metadata AC-4: Arrays remain arrays, numbers remain numeric, dates convert to ISO-8601, and missing sections emit empty objects
#[test]
fn test_ac4_types_arrays_numbers_dates_empty_sections() {
    let jpg = build_sample_jpeg();
    let res = extract_metadata_from_bytes("photo.jpg", &jpg).expect("extract jpeg");

    // Arrays remain arrays
    assert_eq!(
        res.metadata.iptc.get("Keywords"),
        Some(&json!(["rust", "metadata", "exif"]))
    );
    assert_eq!(
        res.metadata.xmp.get("dc:subject"),
        Some(&json!(["landscape", "alps"]))
    );
    assert_eq!(
        res.metadata.exif.get("GPSLatitude"),
        Some(&json!([52, 31, 12.5]))
    );

    // Numeric values remain numeric
    assert_eq!(res.metadata.exif.get("Orientation"), Some(&json!(1)));
    assert_eq!(res.metadata.exif.get("ISOSpeedRatings"), Some(&json!(400)));
    assert_eq!(res.metadata.exif.get("FNumber"), Some(&json!(2.8)));
    assert_eq!(res.metadata.xmp.get("xmp:Rating"), Some(&json!(5)));

    // Dates become ISO-8601 strings
    assert_eq!(
        res.metadata.exif.get("DateTimeOriginal"),
        Some(&json!("2026-05-14T10:30:45"))
    );
    assert_eq!(
        res.metadata.iptc.get("DateCreated"),
        Some(&json!("2026-05-14"))
    );
    assert_eq!(
        res.metadata.xmp.get("xmp:CreateDate"),
        Some(&json!("2026-05-14T10:30:45"))
    );

    // Missing sections emit empty objects {}
    let minimal_jpeg = vec![0xFF, 0xD8, 0xFF, 0xD9];
    let empty_res = extract_metadata_from_bytes("empty.jpg", &minimal_jpeg).expect("minimal jpeg");
    let serialized: Value = serde_json::to_value(&empty_res).unwrap();
    assert_eq!(serialized["metadata"]["exif"], json!({}));
    assert_eq!(serialized["metadata"]["iptc"], json!({}));
    assert_eq!(serialized["metadata"]["xmp"], json!({}));
}

// img-metadata AC-5: Returns valid error JSON on missing CLI argument, missing file, or invalid image data
#[test]
fn test_ac5_error_handling_valid_json() {
    // Missing CLI argument
    let (out_no_arg, code_no_arg) = run_cli_with_args(["wasm_img-metadata"]);
    assert_ne!(code_no_arg, 0);
    let v_no_arg: Value = serde_json::from_str(&out_no_arg).expect("valid error JSON");
    assert_eq!(v_no_arg["success"], json!(false));
    assert!(v_no_arg["error"].as_str().unwrap().contains("Usage:"));

    // Non-existent file
    let (out_missing, code_missing) =
        run_cli_with_args(["wasm_img-metadata", "/nonexistent/path/to/photo.jpg"]);
    assert_ne!(code_missing, 0);
    let v_missing: Value = serde_json::from_str(&out_missing).expect("valid error JSON");
    assert_eq!(v_missing["success"], json!(false));
    assert!(v_missing["error"]
        .as_str()
        .unwrap()
        .contains("Failed to read file"));

    // Invalid/corrupted container bytes
    let err = extract_metadata_from_bytes("bad.bin", b"NOT_AN_IMAGE_FILE").unwrap_err();
    assert!(!err.success);
    assert!(err.error.contains("Unsupported or unrecognized image format"));
}
