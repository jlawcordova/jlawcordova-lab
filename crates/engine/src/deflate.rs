//! Deflate, zlib, gzip and CRC-32 around `miniz_oxide` (spec D2): what the
//! Node CLI got from `node:zlib`. `miniz_oxide` doesn't produce zlib's exact
//! bytes, so gzip sizes may differ from the Node CLI's by a few bytes (the
//! owner accepted this); every stream here is standard and inflates with
//! any reader.

use miniz_oxide::deflate::{compress_to_vec, compress_to_vec_zlib};
use miniz_oxide::inflate::{decompress_to_vec, decompress_to_vec_zlib};

const LEVEL: u8 = 6;

fn crc_table() -> &'static [u32; 256] {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0u32; 256];
        for (n, slot) in table.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *slot = c;
        }
        table
    })
}

/// CRC-32 (IEEE 802.3, as zlib's `crc32`) of `data`.
pub fn crc32(data: &[u8]) -> u32 {
    let table = crc_table();
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

/// A zlib stream (RFC 1950) of `data` at level 6, as `deflateSync`.
pub fn zlib(data: &[u8]) -> Vec<u8> {
    compress_to_vec_zlib(data, LEVEL)
}

/// The bytes of a zlib stream.
pub fn unzlib(data: &[u8]) -> Result<Vec<u8>, String> {
    decompress_to_vec_zlib(data).map_err(|e| format!("bad zlib stream: {:?}", e.status))
}

/// A gzip stream (RFC 1952) of `data` at level 6, as `gzipSync`.
pub fn gzip(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0x03];
    out.extend(compress_to_vec(data, LEVEL));
    out.extend(crc32(data).to_le_bytes());
    out.extend((data.len() as u32).to_le_bytes());
    out
}

/// The bytes of a gzip stream.
pub fn gunzip(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 18 || data[0] != 0x1f || data[1] != 0x8b {
        return Err("not a gzip stream".into());
    }
    if data[2] != 8 {
        return Err("unsupported gzip method".into());
    }
    let flags = data[3];
    let mut pos = 10;
    let trailer = data.len() - 8;
    if flags & 4 != 0 {
        if pos + 2 > trailer {
            return Err("truncated gzip header".into());
        }
        pos += 2 + u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
    }
    for bit in [8u8, 16] {
        if flags & bit != 0 {
            match data.get(pos..trailer).and_then(|s| s.iter().position(|&b| b == 0)) {
                Some(i) => pos += i + 1,
                None => return Err("truncated gzip header".into()),
            }
        }
    }
    if flags & 2 != 0 {
        pos += 2;
    }
    if pos > trailer {
        return Err("truncated gzip header".into());
    }
    let out = decompress_to_vec(&data[pos..trailer]).map_err(|e| format!("bad gzip data: {:?}", e.status))?;
    let crc = u32::from_le_bytes(data[trailer..trailer + 4].try_into().unwrap());
    let size = u32::from_le_bytes(data[trailer + 4..].try_into().unwrap());
    if crc != crc32(&out) {
        return Err("gzip CRC mismatch".into());
    }
    if size != out.len() as u32 {
        return Err("gzip size mismatch".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(n: usize) -> Vec<u8> {
        let mut x = 0x1234_5678u32;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x >> 8) as u8
            })
            .collect()
    }

    #[test]
    fn crc32_check_values() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn zlib_round_trips() {
        for data in [Vec::new(), b"hello".to_vec(), noise(1_000_000)] {
            assert_eq!(unzlib(&zlib(&data)).unwrap(), data);
        }
        assert!(unzlib(b"nope").is_err());
    }

    #[test]
    fn gzip_round_trips() {
        for data in [Vec::new(), b"hello".to_vec(), noise(1_000_000)] {
            let z = gzip(&data);
            assert_eq!(&z[..10], &[0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3]);
            assert_eq!(gunzip(&z).unwrap(), data);
        }
        let mut bad = gzip(b"abc");
        let n = bad.len();
        bad[n - 8] ^= 1;
        assert!(gunzip(&bad).is_err());
        assert!(gunzip(b"short").is_err());
    }

    #[test]
    fn gzip_is_read_by_system_gunzip() {
        let path = std::env::temp_dir().join(format!("pixellab-gzip-{}.gz", std::process::id()));
        std::fs::write(&path, gzip(b"hello pixel lab\n")).unwrap();
        match std::process::Command::new("gunzip").arg("-c").arg(&path).output() {
            Ok(out) => {
                assert!(out.status.success());
                assert_eq!(out.stdout, b"hello pixel lab\n");
            }
            Err(_) => println!("gunzip not found; skipping"),
        }
        let _ = std::fs::remove_file(&path);
    }
}
