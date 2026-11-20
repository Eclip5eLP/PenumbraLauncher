//! Chunk store conventions shared by publisher and launcher (docs/03-content-format.md#chunk-store-server).

/// Content-defined chunking for files >= [`SMALL_FILE`]: min / avg / max chunk size.
pub const CDC_MIN: u32 = 1 << 20;
pub const CDC_AVG: u32 = 4 << 20;
pub const CDC_MAX: u32 = 16 << 20;

/// Files smaller than this are packed together into shared "pack" chunks (ADR-016), so items with thousands
/// of tiny files don't need thousands of HTTP requests.
pub const SMALL_FILE: u64 = 1 << 20;
/// A pack is closed once it reaches this size ...
pub const PACK_MAX: u64 = 4 << 20;
/// ... or, once above this size, at a content-defined boundary (keeps packs stable when files are added).
pub const PACK_MIN: u64 = 1 << 20;

/// BLAKE3 of `data` as lowercase hex.
pub fn hash_hex(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

pub fn is_valid_hash(h: &str) -> bool {
    h.len() == 64 && h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Server path of a chunk, relative to the penumbra root: `chunks/ab/<hash>.bin`.
pub fn chunk_path(hash: &str) -> String {
    format!("chunks/{}/{}.bin", &hash[..2], hash)
}

/// Content-defined pack boundary: true for ~1 in 8 files, decided by the file's own content hash.
pub fn is_pack_boundary(file_hash_hex: &str) -> bool {
    u8::from_str_radix(&file_hash_hex[..2], 16).map(|b| b < 32).unwrap_or(false)
}

/// Decompresses a stored chunk and checks it against its hash.
pub fn decode_chunk(stored: &[u8], expected_hash: &str, expected_len: u64) -> Result<Vec<u8>, ChunkError> {
    let data = zstd::bulk::decompress(stored, expected_len as usize).map_err(|e| ChunkError::Decode(e.to_string()))?;
    if data.len() as u64 != expected_len {
        return Err(ChunkError::Length { expected: expected_len, got: data.len() as u64 });
    }
    if hash_hex(&data) != expected_hash {
        return Err(ChunkError::Hash);
    }
    Ok(data)
}

#[derive(Debug, thiserror::Error)]
pub enum ChunkError {
    #[error("chunk could not be decompressed: {0}")]
    Decode(String),
    #[error("chunk has wrong length (expected {expected}, got {got})")]
    Length { expected: u64, got: u64 },
    #[error("chunk hash mismatch")]
    Hash,
}

/// Version strings become path segments on the server: keep them boring.
pub fn is_valid_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 64
        && !v.starts_with('.')
        && v.bytes().all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
}

pub fn is_valid_item_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_tamper() {
        let data = b"hello chunk".repeat(100);
        let h = hash_hex(&data);
        let stored = zstd::bulk::compress(&data, 3).unwrap();
        assert_eq!(decode_chunk(&stored, &h, data.len() as u64).unwrap(), data);
        assert!(decode_chunk(&stored, &hash_hex(b"other"), data.len() as u64).is_err());
        assert!(decode_chunk(&stored, &h, 5).is_err());
        assert_eq!(chunk_path(&h), format!("chunks/{}/{h}.bin", &h[..2]));
        assert!(is_valid_hash(&h));
    }

    #[test]
    fn version_and_id_rules() {
        assert!(is_valid_version("0.3.14+1.5.1+1.2.51"));
        assert!(is_valid_version("v1.0.0-beta.2"));
        assert!(!is_valid_version("../x"));
        assert!(!is_valid_version("a/b"));
        assert!(is_valid_item_id("gracidea"));
        assert!(!is_valid_item_id("Gracidea"));
    }
}
