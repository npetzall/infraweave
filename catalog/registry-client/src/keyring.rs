//! Combined GPG keyring file content and armored-block parsing.

use std::io;
use std::path::Path;

use pgp::composed::{Deserializable, SignedPublicKey};

use crate::error::ProviderRegistryError;

const ARMOR_PUB_BEGIN: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----";
const ARMOR_PUB_END: &str = "-----END PGP PUBLIC KEY BLOCK-----";

/// In-memory OpenPGP public keyring as a list of ASCII-armored blocks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GpgKeyring {
    blocks: Vec<String>,
}

impl GpgKeyring {
    pub fn new() -> Self {
        Self { blocks: Vec::new() }
    }

    /// Parse armored blocks from a string (e.g. file contents), without I/O.
    pub fn from_armored_text(s: &str) -> Self {
        Self {
            blocks: armored_public_key_blocks(s),
        }
    }

    pub fn read_from(path: impl AsRef<Path>) -> io::Result<Self> {
        let text = std::fs::read_to_string(path.as_ref())?;
        Ok(Self::from_armored_text(&text))
    }

    #[allow(dead_code)] // asserted in unit tests; not used by lib targets otherwise
    pub fn blocks(&self) -> &[String] {
        &self.blocks
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Append armor from `ascii_armor`. Splits multiple `BEGIN PGP PUBLIC KEY` blocks when present.
    pub fn add_armored(&mut self, ascii_armor: impl AsRef<str>) {
        let s = ascii_armor.as_ref().trim();
        if s.is_empty() {
            return;
        }
        let parsed = armored_public_key_blocks(s);
        if parsed.is_empty() {
            self.blocks.push(s.to_string());
        } else {
            self.blocks.extend(parsed);
        }
    }

    /// Same layout as registry keyring files: blocks joined with blank lines.
    pub fn to_file_contents(&self) -> String {
        let mut kr = String::new();
        for block in &self.blocks {
            if !kr.is_empty() {
                kr.push_str("\n\n");
            }
            kr.push_str(block.trim());
        }
        kr
    }

    pub fn iter_signed_public_keys(&self) -> SignedPublicKeyIter<'_> {
        SignedPublicKeyIter {
            inner: self.blocks.iter().enumerate(),
        }
    }
}

/// Parses each stored armored block as a [`SignedPublicKey`].
pub(crate) struct SignedPublicKeyIter<'a> {
    inner: std::iter::Enumerate<std::slice::Iter<'a, String>>,
}

impl<'a> Iterator for SignedPublicKeyIter<'a> {
    type Item = Result<SignedPublicKey, ProviderRegistryError>;

    fn next(&mut self) -> Option<Self::Item> {
        let (i, armor) = self.inner.next()?;
        let key_id = format!("(keyring block {i})");
        Some(
            SignedPublicKey::from_string(armor.trim())
                .map(|(pk, _)| pk)
                .map_err(|source| ProviderRegistryError::ParseGpgPublicKey { key_id, source }),
        )
    }
}

pub(crate) fn armored_public_key_blocks(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while let Some(rel) = s.get(pos..).and_then(|t| t.find(ARMOR_PUB_BEGIN)) {
        let i = pos + rel;
        let Some(tail) = s.get(i..) else {
            break;
        };
        let Some(end_rel) = tail.find(ARMOR_PUB_END) else {
            break;
        };
        let end_byte = end_rel + ARMOR_PUB_END.len();
        if let Some(block) = tail.get(..end_byte) {
            out.push(block.trim().to_string());
        }
        pos = i + end_byte;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_file_contents_joins_blocks() {
        let mut kr = GpgKeyring::new();
        kr.add_armored(
            "-----BEGIN PGP PUBLIC KEY BLOCK-----\nx\n-----END PGP PUBLIC KEY BLOCK-----",
        );
        kr.add_armored(
            "-----BEGIN PGP PUBLIC KEY BLOCK-----\ny\n-----END PGP PUBLIC KEY BLOCK-----",
        );
        assert_eq!(
            kr.to_file_contents(),
            "-----BEGIN PGP PUBLIC KEY BLOCK-----\nx\n-----END PGP PUBLIC KEY BLOCK-----\n\n-----BEGIN PGP PUBLIC KEY BLOCK-----\ny\n-----END PGP PUBLIC KEY BLOCK-----"
        );
    }

    #[test]
    fn from_armored_text_roundtrip() {
        let text = "-----BEGIN PGP PUBLIC KEY BLOCK-----\na\n-----END PGP PUBLIC KEY BLOCK-----\n\n-----BEGIN PGP PUBLIC KEY BLOCK-----\nb\n-----END PGP PUBLIC KEY BLOCK-----";
        let kr = GpgKeyring::from_armored_text(text);
        assert_eq!(kr.blocks().len(), 2);
        assert_eq!(kr.to_file_contents(), text);
    }

    #[test]
    fn armored_public_key_blocks_empty() {
        assert!(armored_public_key_blocks("").is_empty());
        assert!(armored_public_key_blocks("no markers here").is_empty());
    }

    #[test]
    fn armored_public_key_blocks_extracts_one_or_many() {
        let one = "prefix\n-----BEGIN PGP PUBLIC KEY BLOCK-----\nk\n-----END PGP PUBLIC KEY BLOCK-----\ntrailing";
        let blocks = armored_public_key_blocks(one);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("k"));

        let two = "-----BEGIN PGP PUBLIC KEY BLOCK-----\na\n-----END PGP PUBLIC KEY BLOCK-----\n\n-----BEGIN PGP PUBLIC KEY BLOCK-----\nb\n-----END PGP PUBLIC KEY BLOCK-----";
        assert_eq!(armored_public_key_blocks(two).len(), 2);
    }

    #[test]
    fn add_armored_empty_is_noop() {
        let mut kr = GpgKeyring::new();
        kr.add_armored("");
        kr.add_armored("   ");
        assert!(kr.is_empty());
    }

    #[test]
    fn add_armored_without_end_marker_stores_whole_trimmed_string() {
        let mut kr = GpgKeyring::new();
        kr.add_armored("-----BEGIN PGP PUBLIC KEY BLOCK-----\nno end");
        assert_eq!(kr.blocks().len(), 1);
        assert_eq!(kr.blocks()[0], "-----BEGIN PGP PUBLIC KEY BLOCK-----\nno end");
    }
}
