//! High-performance zstd compression and decompression utilities.

/// Compresses a raw byte slice using zstd at level 3 (balanced speed and ratio).
pub fn compress_bytes(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    zstd::encode_all(data, 3)
}

/// Decompresses a zstd-compressed byte slice.
pub fn decompress_bytes(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    zstd::decode_all(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zstd_roundtrip() {
        let original =
            b"Flash search ultrafast indexing with zstd compression test payload data 1234567890";
        let compressed = compress_bytes(original).expect("compression failed");
        let decompressed = decompress_bytes(&compressed).expect("decompression failed");
        assert_eq!(original.as_slice(), decompressed.as_slice());
    }
}
