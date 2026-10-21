//! Deflate, zlib, gzip and CRC-32 around `miniz_oxide` (spec D2): what the
//! Node CLI got from `node:zlib`. `miniz_oxide` doesn't produce zlib's exact
//! bytes, so gzip sizes may differ from the Node CLI's by a few bytes (the
//! owner accepted this); every stream here is standard and inflates with
//! any reader.

/// CRC-32 (IEEE 802.3, as zlib's `crc32`) of `data`.
pub fn crc32(data: &[u8]) -> u32 {
    let _ = data;
    todo!("step 4: CRC-32")
}

/// A zlib stream (RFC 1950) of `data` at level 6, as `deflateSync`.
pub fn zlib(data: &[u8]) -> Vec<u8> {
    let _ = data;
    todo!("step 4: zlib")
}

/// The bytes of a zlib stream.
pub fn unzlib(data: &[u8]) -> Result<Vec<u8>, String> {
    let _ = data;
    todo!("step 4: inflate a zlib stream")
}

/// A gzip stream (RFC 1952) of `data` at level 6, as `gzipSync`.
pub fn gzip(data: &[u8]) -> Vec<u8> {
    let _ = data;
    todo!("step 4: gzip")
}

/// The bytes of a gzip stream.
pub fn gunzip(data: &[u8]) -> Result<Vec<u8>, String> {
    let _ = data;
    todo!("step 4: gunzip")
}
