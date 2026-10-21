//! PNG encode and decode (the encoder ports `encodePng` and `chunk` from
//! cli/lib/engine/preview.mjs): 8-bit RGBA, no interlace. Encoding writes
//! filter 0 on every row and deflates with `deflate::zlib`, as Node's
//! `deflateSync` did; decoding reads any 8-bit RGBA or RGB PNG with the five
//! standard filters, which is what the tests need to
//! compare previews pixel for pixel.

/// An image: width × height pixels, four bytes each (RGBA), row by row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

use crate::deflate::{crc32, unzlib, zlib};

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend(kind);
    out.extend(data);
    let crc = crc32(&out[start..]);
    out.extend(crc.to_be_bytes());
}

/// An RGBA PNG of `image`, scaled up by whole-pixel repetition.
pub fn encode(image: &Image, scale: u32) -> Vec<u8> {
    let w = (image.width * scale) as usize;
    let h = (image.height * scale) as usize;
    let scale = scale as usize;
    let src_w = image.width as usize;
    let stride = w * 4 + 1;
    let mut raw = vec![0u8; stride * h];
    for y in 0..h {
        let sy = y / scale;
        for x in 0..w {
            let from = (sy * src_w + x / scale) * 4;
            let to = y * stride + 1 + x * 4;
            raw[to..to + 4].copy_from_slice(&image.rgba[from..from + 4]);
        }
    }
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend((w as u32).to_be_bytes());
    ihdr.extend((h as u32).to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]);
    let mut out = vec![137, 80, 78, 71, 13, 10, 26, 10];
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Decodes a PNG to RGBA, or says why it can't.
pub fn decode(bytes: &[u8]) -> Result<Image, String> {
    if bytes.len() < 8 || bytes[..8] != [137, 80, 78, 71, 13, 10, 26, 10] {
        return Err("not a PNG".into());
    }
    let mut pos = 8;
    let mut header: Option<(u32, u32, usize)> = None;
    let mut idat = Vec::new();
    let mut ended = false;
    while pos < bytes.len() {
        if pos + 12 > bytes.len() {
            return Err("truncated PNG chunk".into());
        }
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        let end = pos.checked_add(12 + len).filter(|&e| e <= bytes.len()).ok_or("truncated PNG chunk")?;
        let kind = &bytes[pos + 4..pos + 8];
        let data = &bytes[pos + 8..pos + 8 + len];
        let crc = u32::from_be_bytes(bytes[end - 4..end].try_into().unwrap());
        if crc != crc32(&bytes[pos + 4..end - 4]) {
            return Err("PNG chunk CRC mismatch".into());
        }
        match kind {
            b"IHDR" => {
                if len != 13 {
                    return Err("bad PNG header".into());
                }
                let width = u32::from_be_bytes(data[0..4].try_into().unwrap());
                let height = u32::from_be_bytes(data[4..8].try_into().unwrap());
                if data[8] != 8 {
                    return Err("only 8-bit PNGs are supported".into());
                }
                let channels = match data[9] {
                    6 => 4,
                    2 => 3,
                    _ => return Err("only RGBA and RGB PNGs are supported".into()),
                };
                if data[10] != 0 || data[11] != 0 {
                    return Err("bad PNG compression or filter method".into());
                }
                if data[12] != 0 {
                    return Err("interlaced PNGs are not supported".into());
                }
                header = Some((width, height, channels));
            }
            b"IDAT" => idat.extend(data),
            b"IEND" => {
                ended = true;
                break;
            }
            _ => {}
        }
        pos = end;
    }
    let (width, height, channels) = header.ok_or("PNG has no header")?;
    if !ended {
        return Err("PNG has no end".into());
    }
    let raw = unzlib(&idat)?;
    let row = width as usize * channels;
    let expected = (row + 1).checked_mul(height as usize).ok_or("PNG too large")?;
    if raw.len() != expected {
        return Err("PNG data has the wrong size".into());
    }
    let mut pixels = vec![0u8; row * height as usize];
    for y in 0..height as usize {
        let filter = raw[y * (row + 1)];
        let line = &raw[y * (row + 1) + 1..(y + 1) * (row + 1)];
        let (before, rest) = pixels.split_at_mut(y * row);
        let cur = &mut rest[..row];
        let prev: &[u8] = if y == 0 { &[] } else { &before[(y - 1) * row..] };
        for i in 0..row {
            let a = if i >= channels { cur[i - channels] } else { 0 };
            let b = if y > 0 { prev[i] } else { 0 };
            let c = if y > 0 && i >= channels { prev[i - channels] } else { 0 };
            let add = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((a as u16 + b as u16) / 2) as u8,
                4 => paeth(a, b, c),
                _ => return Err("bad PNG filter".into()),
            };
            cur[i] = line[i].wrapping_add(add);
        }
    }
    let rgba = if channels == 4 { pixels } else { pixels.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect() };
    Ok(Image { width, height, rgba })
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (a as i16, b as i16, c as i16);
    let p = ia + ib - ic;
    let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RGBA: [u8; 24] = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 10, 20, 30, 0, 200, 100, 50, 128, 255, 255, 255, 255];
    const NODE_PNG: &str = "89504e470d0a1a0a0000000d49484452000000030000000208060000009d74661a0000001f49444154789c63f8cfc0f01f0c19feff67e0129163389162d4f0ffffffff008ac60c1149ada1300000000049454e44ae426082";
    const NODE_SCALE2_HEAD: &str = "89504e470d0a1a0a0000000d4948445200000006000000040806000000ad044e43";

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    fn sample() -> Image {
        Image { width: 3, height: 2, rgba: RGBA.to_vec() }
    }

    #[test]
    fn decodes_a_png_made_by_node() {
        assert_eq!(decode(&unhex(NODE_PNG)).unwrap(), sample());
    }

    #[test]
    fn encode_matches_node_header_and_pixels() {
        let mine = encode(&sample(), 1);
        let node = unhex(NODE_PNG);
        assert_eq!(mine[..33], node[..33]);
        assert_eq!(decode(&mine).unwrap(), decode(&node).unwrap());
        assert_eq!(encode(&sample(), 2)[..33], unhex(NODE_SCALE2_HEAD)[..]);
    }

    #[test]
    fn round_trip_at_scale_1_and_3() {
        assert_eq!(decode(&encode(&sample(), 1)).unwrap(), sample());
        let big = decode(&encode(&sample(), 3)).unwrap();
        assert_eq!((big.width, big.height), (9, 6));
        for y in 0..6usize {
            for x in 0..9usize {
                let s = ((y / 3) * 3 + x / 3) * 4;
                let d = (y * 9 + x) * 4;
                assert_eq!(big.rgba[d..d + 4], RGBA[s..s + 4]);
            }
        }
    }

    #[test]
    fn decodes_filters_and_rgb() {
        // 2x2 RGB, rows use Sub, Up.
        let raw = [1u8, 10, 20, 30, 1, 1, 1, 2, 5, 5, 5, 5, 5, 5];
        let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10];
        let mut ihdr = Vec::new();
        ihdr.extend(2u32.to_be_bytes());
        ihdr.extend(2u32.to_be_bytes());
        ihdr.extend([8, 2, 0, 0, 0]);
        chunk(&mut png, b"IHDR", &ihdr);
        chunk(&mut png, b"IDAT", &zlib(&raw));
        chunk(&mut png, b"IEND", &[]);
        let img = decode(&png).unwrap();
        assert_eq!(img.rgba, [10, 20, 30, 255, 11, 21, 31, 255, 15, 25, 35, 255, 16, 26, 36, 255]);
        let mut bad = png.clone();
        bad[20] ^= 1;
        assert!(decode(&bad).is_err());
        assert!(decode(b"nope").is_err());
    }
}
