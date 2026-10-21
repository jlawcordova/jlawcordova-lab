//! SHA-256 (FIPS 180-4), for a proposal's `baseHash` and a source's hash
//! (session.mjs `hashText`, which used `node:crypto`).

/// The SHA-256 digest of `data`.
pub fn digest(data: &[u8]) -> [u8; 32] {
    let _ = data;
    todo!("step 4: SHA-256")
}

/// The digest of `data` as lowercase hex, as `createHash('sha256').digest('hex')`.
pub fn hex(data: &[u8]) -> String {
    digest(data).iter().map(|b| format!("{b:02x}")).collect()
}
