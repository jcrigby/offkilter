//! Minimal PNG: 8-bit RGB, no interlace, filter 0 rows, zlib via
//! miniz_oxide.

use crate::raster::Image;

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (n, entry) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *entry = c;
    }
    table
}

fn crc32(table: &[u32; 256], bytes: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in bytes {
        c = table[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    c ^ 0xffff_ffff
}

fn chunk(out: &mut Vec<u8>, table: &[u32; 256], kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(table, &out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

pub fn encode(image: &Image) -> Vec<u8> {
    let table = crc_table();
    let mut raw = Vec::with_capacity(image.height * (image.width * 3 + 1));
    for row in image.data.chunks_exact(image.width * 3) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
    let mut out = Vec::with_capacity(compressed.len() + 64);
    out.extend_from_slice(SIGNATURE);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(image.width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(image.height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, &table, b"IHDR", &ihdr);
    chunk(&mut out, &table, b"IDAT", &compressed);
    chunk(&mut out, &table, b"IEND", &[]);
    out
}

pub fn decode(bytes: &[u8]) -> Result<Image, String> {
    if !bytes.starts_with(SIGNATURE) {
        return Err("not a PNG".into());
    }
    let table = crc_table();
    let mut at = 8;
    let mut size = None;
    let mut idat = Vec::new();
    while at + 12 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let kind = &bytes[at + 4..at + 8];
        let end = at + 8 + len;
        if end + 4 > bytes.len() {
            return Err("truncated chunk".into());
        }
        let data = &bytes[at + 8..end];
        let crc = u32::from_be_bytes(bytes[end..end + 4].try_into().unwrap());
        if crc != crc32(&table, &bytes[at + 4..end]) {
            return Err("bad chunk crc".into());
        }
        match kind {
            b"IHDR" => {
                if data.len() != 13 || data[8] != 8 || data[9] != 2 || data[12] != 0 {
                    return Err("only 8-bit RGB without interlace is read".into());
                }
                let w = u32::from_be_bytes(data[0..4].try_into().unwrap()) as usize;
                let h = u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize;
                size = Some((w, h));
            }
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        at = end + 4;
    }
    let (width, height) = size.ok_or("no IHDR")?;
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&idat)
        .map_err(|e| format!("inflate failed: {e:?}"))?;
    let stride = width * 3 + 1;
    if raw.len() != stride * height {
        return Err("image data has the wrong length".into());
    }
    let mut data = Vec::with_capacity(width * height * 3);
    for row in raw.chunks_exact(stride) {
        if row[0] != 0 {
            return Err("only filter 0 rows are read".into());
        }
        data.extend_from_slice(&row[1..]);
    }
    Ok(Image {
        width,
        height,
        data,
    })
}
