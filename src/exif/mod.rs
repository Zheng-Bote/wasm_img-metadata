use std::collections::HashSet;

use serde_json::{Map, Number, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endian {
    Little,
    Big,
}

impl Endian {
    fn read_u16(self, b: &[u8]) -> u16 {
        match self {
            Endian::Little => u16::from_le_bytes([b[0], b[1]]),
            Endian::Big => u16::from_be_bytes([b[0], b[1]]),
        }
    }

    fn read_i16(self, b: &[u8]) -> i16 {
        match self {
            Endian::Little => i16::from_le_bytes([b[0], b[1]]),
            Endian::Big => i16::from_be_bytes([b[0], b[1]]),
        }
    }

    fn read_u32(self, b: &[u8]) -> u32 {
        match self {
            Endian::Little => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            Endian::Big => u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
        }
    }

    fn read_i32(self, b: &[u8]) -> i32 {
        match self {
            Endian::Little => i32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            Endian::Big => i32::from_be_bytes([b[0], b[1], b[2], b[3]]),
        }
    }

    fn read_f32(self, b: &[u8]) -> f32 {
        match self {
            Endian::Little => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            Endian::Big => f32::from_be_bytes([b[0], b[1], b[2], b[3]]),
        }
    }

    fn read_f64(self, b: &[u8]) -> f64 {
        let arr: [u8; 8] = [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]];
        match self {
            Endian::Little => f64::from_le_bytes(arr),
            Endian::Big => f64::from_be_bytes(arr),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IfdKind {
    Primary,
    Exif,
    Gps,
    Interop,
    SubIfd,
}

#[derive(Debug, Default)]
pub struct TiffParseOutcome {
    pub exif: Map<String, Value>,
    pub embedded_iptc: Vec<Vec<u8>>,
    pub embedded_xmp: Vec<String>,
}

pub fn parse_tiff_exif(data: &[u8]) -> Result<TiffParseOutcome, String> {
    if data.len() < 8 {
        return Err("EXIF/TIFF buffer too short".to_string());
    }

    // Some EXIF chunks still carry the "Exif\0\0" prefix
    let tiff = if data.starts_with(b"Exif\0\0") && data.len() >= 14 {
        &data[6..]
    } else {
        data
    };

    let endian = match &tiff[0..4] {
        b"II*\0" => Endian::Little,
        b"MM\0*" => Endian::Big,
        _ => return Err("Invalid TIFF byte-order header".to_string()),
    };

    let first_ifd_offset = endian.read_u32(&tiff[4..8]) as usize;
    let mut outcome = TiffParseOutcome::default();
    let mut visited = HashSet::new();

    parse_ifd_chain(
        tiff,
        endian,
        first_ifd_offset,
        IfdKind::Primary,
        &mut visited,
        &mut outcome,
    );

    Ok(outcome)
}

fn parse_ifd_chain(
    tiff: &[u8],
    endian: Endian,
    start_offset: usize,
    kind: IfdKind,
    visited: &mut HashSet<usize>,
    outcome: &mut TiffParseOutcome,
) {
    let mut current_offset = start_offset;

    while current_offset != 0 && current_offset + 2 <= tiff.len() {
        if !visited.insert(current_offset) {
            break;
        }

        let entry_count = endian.read_u16(&tiff[current_offset..current_offset + 2]) as usize;
        let entries_start = current_offset + 2;
        let Some(entries_end) = entries_start.checked_add(entry_count.saturating_mul(12)) else {
            break;
        };
        if entries_end > tiff.len() {
            break;
        }

        let mut sub_ifds: Vec<(usize, IfdKind)> = Vec::new();

        for i in 0..entry_count {
            let entry_pos = entries_start + i * 12;
            let entry = &tiff[entry_pos..entry_pos + 12];
            let tag_id = endian.read_u16(&entry[0..2]);
            let field_type = endian.read_u16(&entry[2..4]);
            let count = endian.read_u32(&entry[4..8]) as usize;
            let raw_val_offset = &entry[8..12];

            // Handle sub-IFD pointers and embedded XMP / IPTC packets
            if kind != IfdKind::Gps {
                match tag_id {
                    // ExifOffset
                    0x8769 => {
                        for ptr in extract_offsets(tiff, endian, field_type, count, raw_val_offset) {
                            sub_ifds.push((ptr, IfdKind::Exif));
                        }
                        continue;
                    }
                    // GPSInfo
                    0x8825 => {
                        for ptr in extract_offsets(tiff, endian, field_type, count, raw_val_offset) {
                            sub_ifds.push((ptr, IfdKind::Gps));
                        }
                        continue;
                    }
                    // InteropOffset
                    0xA005 => {
                        for ptr in extract_offsets(tiff, endian, field_type, count, raw_val_offset) {
                            sub_ifds.push((ptr, IfdKind::Interop));
                        }
                        continue;
                    }
                    // SubIFDs
                    0x014A => {
                        for ptr in extract_offsets(tiff, endian, field_type, count, raw_val_offset) {
                            sub_ifds.push((ptr, IfdKind::SubIfd));
                        }
                        continue;
                    }
                    // Embedded XMP packet inside TIFF IFD (tag 700 / 0x02BC)
                    0x02BC => {
                        if let Some(bytes) = resolve_raw_bytes(tiff, endian, field_type, count, raw_val_offset) {
                            let s = String::from_utf8_lossy(bytes).trim_matches('\0').to_string();
                            if !s.is_empty() {
                                outcome.embedded_xmp.push(s);
                            }
                        }
                        continue;
                    }
                    // Embedded IPTC-NAA inside TIFF IFD (tag 33723 / 0x83BB)
                    0x83BB => {
                        if let Some(bytes) = resolve_raw_bytes(tiff, endian, field_type, count, raw_val_offset) {
                            if !bytes.is_empty() {
                                outcome.embedded_iptc.push(bytes.to_vec());
                            }
                        }
                        continue;
                    }
                    // Embedded Photoshop ImageResources inside TIFF IFD (tag 34377 / 0x8649)
                    0x8649 => {
                        if let Some(bytes) = resolve_raw_bytes(tiff, endian, field_type, count, raw_val_offset) {
                            let mut tmp = crate::container::ExtractedPayloads::default();
                            crate::container::extract_photoshop_irb(bytes, &mut tmp);
                            outcome.embedded_iptc.extend(tmp.iptc_chunks);
                            outcome.embedded_xmp.extend(tmp.xmp_chunks);
                        }
                        continue;
                    }
                    _ => {}
                }
            }

            let Some(val) = decode_tag_value(tiff, endian, kind, tag_id, field_type, count, raw_val_offset) else {
                continue;
            };

            let tag_name = lookup_tag_name(kind, tag_id);
            outcome.exif.insert(tag_name.clone(), val.clone());

            // When Model (0x0110) is present, also populate CameraModel if not already set so both
            // canonical EXIF "Model" and "CameraModel" consumers are satisfied cleanly.
            if tag_id == 0x0110 && kind != IfdKind::Gps && !outcome.exif.contains_key("CameraModel") {
                outcome.exif.insert("CameraModel".to_string(), val);
            }
        }

        for (ptr, sub_kind) in sub_ifds {
            parse_ifd_chain(tiff, endian, ptr, sub_kind, visited, outcome);
        }

        if entries_end + 4 <= tiff.len() {
            current_offset = endian.read_u32(&tiff[entries_end..entries_end + 4]) as usize;
        } else {
            break;
        }
    }
}

fn type_unit_size(field_type: u16) -> Option<usize> {
    match field_type {
        1 | 2 | 6 | 7 => Some(1), // BYTE, ASCII, SBYTE, UNDEFINED
        3 | 8 => Some(2),         // SHORT, SSHORT
        4 | 9 | 11 | 13 => Some(4), // LONG, SLONG, FLOAT, IFD
        5 | 10 | 12 => Some(8),   // RATIONAL, SRATIONAL, DOUBLE
        _ => None,
    }
}

fn resolve_raw_bytes<'a>(
    tiff: &'a [u8],
    endian: Endian,
    field_type: u16,
    count: usize,
    raw_val_offset: &'a [u8],
) -> Option<&'a [u8]> {
    let unit = type_unit_size(field_type).unwrap_or(1);
    let total_bytes = unit.checked_mul(count)?;
    if total_bytes <= 4 {
        Some(&raw_val_offset[..total_bytes])
    } else {
        let offset = endian.read_u32(raw_val_offset) as usize;
        let end = offset.checked_add(total_bytes)?;
        tiff.get(offset..end)
    }
}

fn extract_offsets(
    tiff: &[u8],
    endian: Endian,
    field_type: u16,
    count: usize,
    raw_val_offset: &[u8],
) -> Vec<usize> {
    let Some(bytes) = resolve_raw_bytes(tiff, endian, field_type, count, raw_val_offset) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    match field_type {
        3 => {
            for chunk in bytes.chunks_exact(2) {
                out.push(endian.read_u16(chunk) as usize);
            }
        }
        4 | 13 => {
            for chunk in bytes.chunks_exact(4) {
                out.push(endian.read_u32(chunk) as usize);
            }
        }
        _ => {}
    }
    out
}

fn decode_tag_value(
    tiff: &[u8],
    endian: Endian,
    kind: IfdKind,
    tag_id: u16,
    field_type: u16,
    count: usize,
    raw_val_offset: &[u8],
) -> Option<Value> {
    let bytes = resolve_raw_bytes(tiff, endian, field_type, count, raw_val_offset)?;

    match field_type {
        // ASCII
        2 => {
            let trimmed = bytes.strip_suffix(&[0]).unwrap_or(bytes);
            // Check for multi-string ASCII separated by internal NUL bytes
            let parts: Vec<&[u8]> = trimmed
                .split(|&b| b == 0)
                .filter(|s| !s.is_empty())
                .collect();
            if parts.len() <= 1 {
                let s = String::from_utf8_lossy(trimmed).trim_matches('\0').to_string();
                Some(normalize_date_or_value(&s))
            } else {
                let arr: Vec<Value> = parts
                    .into_iter()
                    .map(|p| {
                        let s = String::from_utf8_lossy(p).to_string();
                        normalize_date_or_value(&s)
                    })
                    .collect();
                Some(Value::Array(arr))
            }
        }
        // BYTE
        1 => {
            // XPTitle / XPComment / XPAuthor / XPKeywords / XPSubject (UTF-16LE in IFD0)
            if (0x9C9B..=0x9C9F).contains(&tag_id) && bytes.len() >= 2 && bytes.len() % 2 == 0 {
                let u16s: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .take_while(|&ch| ch != 0)
                    .collect();
                if let Ok(s) = String::from_utf16(&u16s) {
                    return Some(Value::String(s));
                }
            }
            let nums: Vec<Value> = bytes.iter().map(|&b| Value::from(b)).collect();
            Some(collapse_single(nums))
        }
        // SHORT
        3 => {
            let nums: Vec<Value> = bytes
                .chunks_exact(2)
                .map(|c| Value::from(endian.read_u16(c)))
                .collect();
            Some(collapse_single(nums))
        }
        // LONG / IFD
        4 | 13 => {
            let nums: Vec<Value> = bytes
                .chunks_exact(4)
                .map(|c| Value::from(endian.read_u32(c)))
                .collect();
            Some(collapse_single(nums))
        }
        // RATIONAL (u32 / u32)
        5 => {
            let nums: Vec<Value> = bytes
                .chunks_exact(8)
                .map(|c| {
                    let num = endian.read_u32(&c[0..4]);
                    let den = endian.read_u32(&c[4..8]);
                    rational_to_json(num as f64, den as f64, num as i64, den as i64)
                })
                .collect();
            Some(collapse_single(nums))
        }
        // SBYTE
        6 => {
            let nums: Vec<Value> = bytes.iter().map(|&b| Value::from(b as i8)).collect();
            Some(collapse_single(nums))
        }
        // UNDEFINED
        7 => Some(decode_undefined_tag(kind, tag_id, bytes)),
        // SSHORT
        8 => {
            let nums: Vec<Value> = bytes
                .chunks_exact(2)
                .map(|c| Value::from(endian.read_i16(c)))
                .collect();
            Some(collapse_single(nums))
        }
        // SLONG
        9 => {
            let nums: Vec<Value> = bytes
                .chunks_exact(4)
                .map(|c| Value::from(endian.read_i32(c)))
                .collect();
            Some(collapse_single(nums))
        }
        // SRATIONAL (i32 / i32)
        10 => {
            let nums: Vec<Value> = bytes
                .chunks_exact(8)
                .map(|c| {
                    let num = endian.read_i32(&c[0..4]);
                    let den = endian.read_i32(&c[4..8]);
                    rational_to_json(num as f64, den as f64, num as i64, den as i64)
                })
                .collect();
            Some(collapse_single(nums))
        }
        // FLOAT
        11 => {
            let nums: Vec<Value> = bytes
                .chunks_exact(4)
                .map(|c| float_to_json(endian.read_f32(c) as f64))
                .collect();
            Some(collapse_single(nums))
        }
        // DOUBLE
        12 => {
            let nums: Vec<Value> = bytes
                .chunks_exact(8)
                .map(|c| float_to_json(endian.read_f64(c)))
                .collect();
            Some(collapse_single(nums))
        }
        // Any unknown future TIFF type: preserve raw byte array so nothing is discarded
        _ => {
            let nums: Vec<Value> = bytes.iter().map(|&b| Value::from(b)).collect();
            Some(collapse_single(nums))
        }
    }
}

fn decode_undefined_tag(_kind: IfdKind, tag_id: u16, bytes: &[u8]) -> Value {
    match tag_id {
        // ExifVersion (0x9000), FlashpixVersion (0xA000), InteropVersion (0x0002)
        0x9000 | 0xA000 => {
            if let Ok(s) = std::str::from_utf8(bytes) {
                return Value::String(s.trim_matches('\0').to_string());
            }
        }
        // UserComment (0x9286): 8-byte character code prefix
        0x9286 if bytes.len() >= 8 => {
            let prefix = &bytes[0..8];
            let body = &bytes[8..];
            if prefix.starts_with(b"ASCII\0\0\0") || prefix == [0; 8] {
                return Value::String(String::from_utf8_lossy(body).trim_matches('\0').to_string());
            } else if prefix.starts_with(b"UNICODE\0") && body.len() % 2 == 0 {
                let u16s: Vec<u16> = body
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .take_while(|&ch| ch != 0)
                    .collect();
                if let Ok(s) = String::from_utf16(&u16s) {
                    return Value::String(s);
                }
            }
        }
        _ => {}
    }

    // If printable ASCII, emit string; otherwise emit numeric byte array (or single number)
    let trimmed = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    if !trimmed.is_empty() && trimmed.iter().all(|&b| (0x20..=0x7E).contains(&b)) {
        Value::String(String::from_utf8_lossy(trimmed).to_string())
    } else {
        let arr: Vec<Value> = bytes.iter().map(|&b| Value::from(b)).collect();
        collapse_single(arr)
    }
}

fn rational_to_json(num_f: f64, den_f: f64, num_i: i64, den_i: i64) -> Value {
    if den_i == 0 {
        return Value::from(0);
    }
    if den_i == 1 {
        return Value::from(num_i);
    }
    if num_i % den_i == 0 {
        return Value::from(num_i / den_i);
    }
    let val = num_f / den_f;
    // Round to 6 decimal places to avoid binary floating point noise while remaining numeric
    let rounded = (val * 1_000_000.0).round() / 1_000_000.0;
    float_to_json(rounded)
}

fn float_to_json(v: f64) -> Value {
    if let Some(n) = Number::from_f64(v) {
        Value::Number(n)
    } else {
        Value::from(0)
    }
}

fn collapse_single(mut items: Vec<Value>) -> Value {
    if items.len() == 1 {
        items.pop().unwrap()
    } else {
        Value::Array(items)
    }
}

/// Converts EXIF date/time strings (`YYYY:MM:DD HH:MM:SS` or `YYYY:MM:DD`) into ISO-8601
/// (`YYYY-MM-DDTHH:MM:SS` or `YYYY-MM-DD`). Preserves non-date strings as-is.
pub fn normalize_date_or_value(s: &str) -> Value {
    let trimmed = s.trim();
    if let Some(iso) = try_exif_date_to_iso8601(trimmed) {
        return Value::String(iso);
    }
    Value::String(trimmed.to_string())
}

pub fn try_exif_date_to_iso8601(s: &str) -> Option<String> {
    let b = s.as_bytes();
    // Pattern: YYYY:MM:DD HH:MM:SS[...] or YYYY:MM:DDTHH:MM:SS[...]
    if b.len() >= 19
        && b[0..4].iter().all(|c| c.is_ascii_digit())
        && (b[4] == b':' || b[4] == b'-')
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && (b[7] == b':' || b[7] == b'-')
        && b[8..10].iter().all(|c| c.is_ascii_digit())
        && (b[10] == b' ' || b[10] == b'T')
        && b[11..13].iter().all(|c| c.is_ascii_digit())
        && b[13] == b':'
        && b[14..16].iter().all(|c| c.is_ascii_digit())
        && b[16] == b':'
        && b[17..19].iter().all(|c| c.is_ascii_digit())
    {
        let date_part = format!("{}-{}-{}", &s[0..4], &s[5..7], &s[8..10]);
        let time_part = &s[11..19];
        let suffix = s[19..].trim();
        return Some(format!("{date_part}T{time_part}{suffix}"));
    }

    // Pattern: YYYY:MM:DD (10 chars)
    if b.len() == 10
        && b[0..4].iter().all(|c| c.is_ascii_digit())
        && b[4] == b':'
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[7] == b':'
        && b[8..10].iter().all(|c| c.is_ascii_digit())
    {
        return Some(format!("{}-{}-{}", &s[0..4], &s[5..7], &s[8..10]));
    }

    None
}

fn lookup_tag_name(kind: IfdKind, tag_id: u16) -> String {
    if kind == IfdKind::Gps {
        let name = match tag_id {
            0x0000 => Some("GPSVersionID"),
            0x0001 => Some("GPSLatitudeRef"),
            0x0002 => Some("GPSLatitude"),
            0x0003 => Some("GPSLongitudeRef"),
            0x0004 => Some("GPSLongitude"),
            0x0005 => Some("GPSAltitudeRef"),
            0x0006 => Some("GPSAltitude"),
            0x0007 => Some("GPSTimeStamp"),
            0x0008 => Some("GPSSatellites"),
            0x0009 => Some("GPSStatus"),
            0x000A => Some("GPSMeasureMode"),
            0x000B => Some("GPSDOP"),
            0x000C => Some("GPSSpeedRef"),
            0x000D => Some("GPSSpeed"),
            0x000E => Some("GPSTrackRef"),
            0x000F => Some("GPSTrack"),
            0x0010 => Some("GPSImgDirectionRef"),
            0x0011 => Some("GPSImgDirection"),
            0x0012 => Some("GPSMapDatum"),
            0x0013 => Some("GPSDestLatitudeRef"),
            0x0014 => Some("GPSDestLatitude"),
            0x0015 => Some("GPSDestLongitudeRef"),
            0x0016 => Some("GPSDestLongitude"),
            0x0017 => Some("GPSDestBearingRef"),
            0x0018 => Some("GPSDestBearing"),
            0x0019 => Some("GPSDestDistanceRef"),
            0x001A => Some("GPSDestDistance"),
            0x001B => Some("GPSProcessingMethod"),
            0x001C => Some("GPSAreaInformation"),
            0x001D => Some("GPSDateStamp"),
            0x001E => Some("GPSDifferential"),
            0x001F => Some("GPSHPositioningError"),
            _ => None,
        };
        return name
            .map(str::to_string)
            .unwrap_or_else(|| format!("GPSTag_0x{tag_id:04X}"));
    }

    if kind == IfdKind::Interop {
        let name = match tag_id {
            0x0001 => Some("InteropIndex"),
            0x0002 => Some("InteropVersion"),
            0x1000 => Some("RelatedImageFileFormat"),
            0x1001 => Some("RelatedImageWidth"),
            0x1002 => Some("RelatedImageLength"),
            _ => None,
        };
        if let Some(n) = name {
            return n.to_string();
        }
    }

    let known = match tag_id {
        0x00FE => Some("NewSubfileType"),
        0x00FF => Some("SubfileType"),
        0x0100 => Some("ImageWidth"),
        0x0101 => Some("ImageLength"),
        0x0102 => Some("BitsPerSample"),
        0x0103 => Some("Compression"),
        0x0106 => Some("PhotometricInterpretation"),
        0x010A => Some("FillOrder"),
        0x010D => Some("DocumentName"),
        0x010E => Some("ImageDescription"),
        0x010F => Some("Make"),
        0x0110 => Some("Model"),
        0x0111 => Some("StripOffsets"),
        0x0112 => Some("Orientation"),
        0x0115 => Some("SamplesPerPixel"),
        0x0116 => Some("RowsPerStrip"),
        0x0117 => Some("StripByteCounts"),
        0x011A => Some("XResolution"),
        0x011B => Some("YResolution"),
        0x011C => Some("PlanarConfiguration"),
        0x0128 => Some("ResolutionUnit"),
        0x012D => Some("TransferFunction"),
        0x0131 => Some("Software"),
        0x0132 => Some("DateTime"),
        0x013B => Some("Artist"),
        0x013E => Some("WhitePoint"),
        0x013F => Some("PrimaryChromaticities"),
        0x0201 => Some("JPEGInterchangeFormat"),
        0x0202 => Some("JPEGInterchangeFormatLength"),
        0x0211 => Some("YCbCrCoefficients"),
        0x0212 => Some("YCbCrSubSampling"),
        0x0213 => Some("YCbCrPositioning"),
        0x0214 => Some("ReferenceBlackWhite"),
        0x8298 => Some("Copyright"),
        0x829A => Some("ExposureTime"),
        0x829D => Some("FNumber"),
        0x8822 => Some("ExposureProgram"),
        0x8824 => Some("SpectralSensitivity"),
        0x8827 => Some("ISOSpeedRatings"),
        0x8828 => Some("OECF"),
        0x8830 => Some("SensitivityType"),
        0x8831 => Some("StandardOutputSensitivity"),
        0x8832 => Some("RecommendedExposureIndex"),
        0x8833 => Some("ISOSpeed"),
        0x9000 => Some("ExifVersion"),
        0x9003 => Some("DateTimeOriginal"),
        0x9004 => Some("DateTimeDigitized"),
        0x9010 => Some("OffsetTime"),
        0x9011 => Some("OffsetTimeOriginal"),
        0x9012 => Some("OffsetTimeDigitized"),
        0x9101 => Some("ComponentsConfiguration"),
        0x9102 => Some("CompressedBitsPerPixel"),
        0x9201 => Some("ShutterSpeedValue"),
        0x9202 => Some("ApertureValue"),
        0x9203 => Some("BrightnessValue"),
        0x9204 => Some("ExposureBiasValue"),
        0x9205 => Some("MaxApertureValue"),
        0x9206 => Some("SubjectDistance"),
        0x9207 => Some("MeteringMode"),
        0x9208 => Some("LightSource"),
        0x9209 => Some("Flash"),
        0x920A => Some("FocalLength"),
        0x9214 => Some("SubjectArea"),
        0x927C => Some("MakerNote"),
        0x9286 => Some("UserComment"),
        0x9290 => Some("SubSecTime"),
        0x9291 => Some("SubSecTimeOriginal"),
        0x9292 => Some("SubSecTimeDigitized"),
        0x9C9B => Some("XPTitle"),
        0x9C9C => Some("XPComment"),
        0x9C9D => Some("XPAuthor"),
        0x9C9E => Some("XPKeywords"),
        0x9C9F => Some("XPSubject"),
        0xA000 => Some("FlashpixVersion"),
        0xA001 => Some("ColorSpace"),
        0xA002 => Some("PixelXDimension"),
        0xA003 => Some("PixelYDimension"),
        0xA004 => Some("RelatedSoundFile"),
        0xA20B => Some("FlashEnergy"),
        0xA20C => Some("SpatialFrequencyResponse"),
        0xA20E => Some("FocalPlaneXResolution"),
        0xA20F => Some("FocalPlaneYResolution"),
        0xA210 => Some("FocalPlaneResolutionUnit"),
        0xA214 => Some("SubjectLocation"),
        0xA215 => Some("ExposureIndex"),
        0xA217 => Some("SensingMethod"),
        0xA300 => Some("FileSource"),
        0xA301 => Some("SceneType"),
        0xA302 => Some("CFAPattern"),
        0xA401 => Some("CustomRendered"),
        0xA402 => Some("ExposureMode"),
        0xA403 => Some("WhiteBalance"),
        0xA404 => Some("DigitalZoomRatio"),
        0xA405 => Some("FocalLengthIn35mmFilm"),
        0xA406 => Some("SceneCaptureType"),
        0xA407 => Some("GainControl"),
        0xA408 => Some("Contrast"),
        0xA409 => Some("Saturation"),
        0xA40A => Some("Sharpness"),
        0xA40B => Some("DeviceSettingDescription"),
        0xA40C => Some("SubjectDistanceRange"),
        0xA420 => Some("ImageUniqueID"),
        0xA430 => Some("CameraOwnerName"),
        0xA431 => Some("BodySerialNumber"),
        0xA432 => Some("LensSpecification"),
        0xA433 => Some("LensMake"),
        0xA434 => Some("LensModel"),
        0xA435 => Some("LensSerialNumber"),
        _ => None,
    };

    known
        .map(str::to_string)
        .unwrap_or_else(|| format!("Tag_0x{tag_id:04X}"))
}
