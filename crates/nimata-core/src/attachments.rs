//! Attachment metadata helpers. File storage itself arrives in Stage 8.

use sha2::{Digest, Sha256};

/// Content hash recorded for every attachment, as `sha256:<lowercase hex>`.
/// Identical files share a hash, which allows deduplication.
pub fn content_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_standard_sha256_test_vector() {
        assert_eq!(
            content_hash(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn identical_content_has_identical_hashes() {
        assert_eq!(content_hash(b"# Notes"), content_hash(b"# Notes"));
        assert_ne!(content_hash(b"# Notes"), content_hash(b"# notes"));
    }
}
