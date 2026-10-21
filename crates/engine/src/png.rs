//! PNG encode and decode (the encoder ports `encodePng` and `chunk` from
//! cli/lib/engine/preview.mjs): 8-bit RGBA, no interlace. Encoding writes
//! filter 0 on every row and deflates with `deflate::zlib`, as Node's
//! `deflateSync` did; decoding reads any 8-bit RGBA or RGB PNG with the five
//! standard filters, which is what the tests and the parity check need to
//! compare previews pixel for pixel.

/// An image: width × height pixels, four bytes each (RGBA), row by row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// An RGBA PNG of `image`, scaled up by whole-pixel repetition.
pub fn encode(image: &Image, scale: u32) -> Vec<u8> {
    let _ = (image, scale);
    todo!("step 4: PNG encode")
}

/// Decodes a PNG to RGBA, or says why it can't.
pub fn decode(bytes: &[u8]) -> Result<Image, String> {
    let _ = bytes;
    todo!("step 4: PNG decode")
}
