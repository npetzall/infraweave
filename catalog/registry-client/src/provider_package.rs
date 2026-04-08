//! On-disk layout for downloaded provider packages and SHA256SUMS + detached GPG verification.

use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::PathBuf;

use pgp::composed::{Deserializable, DetachedSignature};
use sha2::{Digest, Sha256};

use crate::error::{ProviderRegistryError, ShasumMismatchDetail};
use crate::keyring::GpgKeyring;

/// Registry logical filename paired with its on-disk path (e.g. zip basename + download location).
#[derive(Debug, Clone)]
pub struct FileArtifact {
    pub filename: String,
    pub path: PathBuf,
}

/// On-disk layout for one platform after download (paths relative to the chosen base directory).
#[derive(Debug, Clone)]
pub struct ProviderPackage {
    pub provider: FileArtifact,
    pub shasum: String,
    pub shasums: FileArtifact,
    pub signature: FileArtifact,
    pub keyring: FileArtifact,
}

impl ProviderPackage {
    fn provider_sha256(&self) -> std::io::Result<String> {
        let mut file = BufReader::new(File::open(&self.provider.path)?);
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let digest = hasher.finalize();
        Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
    }

    fn hex_eq_ignore_case(a: &str, b: &str) -> bool {
        a.len() == b.len()
            && a.bytes()
                .zip(b.bytes())
                .all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
    }

    /// Verifies the detached GPG signature over `SHA256SUMS` using the armored keys at
    /// [`Self::keyring`], then returns the manifest SHA256 hex for [`Self::provider`]'s
    /// [`FileArtifact::filename`] (lowercase).
    ///
    /// Returns [`ProviderRegistryError::GpgSignatureVerificationFailed`] if no key verifies the
    /// signature, and [`ProviderRegistryError::ShasumNotInManifest`] if the file is absent from
    /// the signed manifest.
    fn shasum_from_file(&self) -> Result<String, ProviderRegistryError> {
        let filename = self.provider.filename.as_str();

        let shasums = std::fs::read(&self.shasums.path)?;
        let sig = std::fs::read(&self.signature.path)?;

        let keyring = GpgKeyring::read_from(&self.keyring.path)?;

        if keyring.is_empty() {
            return Err(ProviderRegistryError::NoGpgPublicKeys);
        }

        let (detached_sig, _) = DetachedSignature::from_reader_single(Cursor::new(sig.as_slice()))
            .map_err(|source| ProviderRegistryError::ParseDetachedSignature { source })?;
        let mut verified = false;
        for pk in keyring.iter_signed_public_keys() {
            let pk = pk?;
            if detached_sig.verify(&pk, shasums.as_slice()).is_ok() {
                verified = true;
                break;
            }
        }
        if !verified {
            return Err(ProviderRegistryError::GpgSignatureVerificationFailed);
        }

        let sums_text = std::str::from_utf8(shasums.as_slice())
            .map_err(|_| ProviderRegistryError::InvalidShasumsUtf8)?;

        for line in sums_text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let Some(hash) = parts.next() else {
                continue;
            };
            let Some(name_field) = parts.next() else {
                continue;
            };
            let name = name_field.trim_start_matches('*');
            if name == filename {
                return Ok(hash.to_ascii_lowercase());
            }
        }

        Err(ProviderRegistryError::ShasumNotInManifest {
            filename: filename.to_owned(),
        })
    }

    /// Verifies the detached GPG signature over `SHA256SUMS`, that registry metadata matches the signed manifest line,
    /// then that the zip digest matches that manifest line.
    ///
    /// On success, returns the provider zip and checksum sidecar files as [`FileArtifact`] entries.
    pub(crate) fn validate(&self) -> Result<Vec<FileArtifact>, ProviderRegistryError> {
        let filename = self.provider.filename.clone();

        let from_sums = self.shasum_from_file()?;
        if !Self::hex_eq_ignore_case(self.shasum.trim(), &from_sums) {
            return Err(ProviderRegistryError::ShasumMismatch {
                filename: filename.clone(),
                detail: ShasumMismatchDetail::RegistryVsShasumsFile,
            });
        }

        let from_zip = self.provider_sha256()?;
        if !Self::hex_eq_ignore_case(&from_zip, &from_sums) {
            return Err(ProviderRegistryError::ShasumMismatch {
                filename: filename.clone(),
                detail: ShasumMismatchDetail::VsShasumsFile,
            });
        }

        Ok(vec![
            self.provider.clone(),
            self.shasums.clone(),
            self.signature.clone(),
            self.keyring.clone(),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shasum_line_parses_star_prefix() {
        let sums_text = "deadbeef *foo.zip\n";
        let filename = "foo.zip";
        let found = (|| {
            for line in sums_text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let mut parts = line.split_whitespace();
                let hash = parts.next()?;
                let name_field = parts.next()?;
                let name = name_field.trim_start_matches('*');
                if name == filename {
                    return Some(hash.to_ascii_lowercase());
                }
            }
            None
        })();
        assert_eq!(found.as_deref(), Some("deadbeef"));
    }

    #[test]
    fn shasum_from_file_requires_keys() {
        let dir = tempfile::tempdir().unwrap();
        let shasums_path = dir.path().join("SHA256SUMS");
        std::fs::write(&shasums_path, "deadbeef *foo.zip\n").unwrap();
        let sig_path = dir.path().join("SHA256SUMS.sig");
        std::fs::write(&sig_path, []).unwrap();
        let keyring_path = dir.path().join("empty.asc");
        std::fs::write(&keyring_path, "").unwrap();
        let pkg = ProviderPackage {
            provider: FileArtifact {
                filename: "foo.zip".to_string(),
                path: dir.path().join("foo.zip"),
            },
            shasum: String::new(),
            shasums: FileArtifact {
                filename: "SHA256SUMS".to_string(),
                path: shasums_path,
            },
            signature: FileArtifact {
                filename: "SHA256SUMS.sig".to_string(),
                path: sig_path,
            },
            keyring: FileArtifact {
                filename: "empty.asc".to_string(),
                path: keyring_path,
            },
        };
        assert!(matches!(
            pkg.shasum_from_file(),
            Err(ProviderRegistryError::NoGpgPublicKeys)
        ));
    }
}
