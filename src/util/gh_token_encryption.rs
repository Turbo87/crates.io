use aes_gcm::aead::{Aead, AeadCore, OsRng};
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use anyhow::{Context, Result};
use secrecy::SecretString;

/// A struct that encapsulates GitHub token encryption and decryption
/// using AES-256-GCM.
pub struct GitHubTokenEncryption {
    cipher: Aes256Gcm,
}

impl GitHubTokenEncryption {
    /// Creates a new [GitHubTokenEncryption] instance with the provided cipher
    pub fn new(cipher: Aes256Gcm) -> Self {
        Self { cipher }
    }

    /// Creates a new [GitHubTokenEncryption] instance with a cipher for testing
    /// purposes.
    #[cfg(any(test, debug_assertions))]
    pub fn for_testing() -> Self {
        let test_key = b"test_key_32_bytes_long_for_tests";
        Self::new(Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(test_key)))
    }

    /// Creates a new [GitHubTokenEncryption] instance from the environment
    ///
    /// Reads the `GITHUB_TOKEN_ENCRYPTION_KEY` environment variable, which
    /// should be a 64-character hex string (32 bytes when decoded).
    pub fn from_environment() -> Result<Self> {
        let gh_token_key = std::env::var("GITHUB_TOKEN_ENCRYPTION_KEY")
            .context("GITHUB_TOKEN_ENCRYPTION_KEY environment variable not set")?;

        if gh_token_key.len() != 64 {
            anyhow::bail!("GITHUB_TOKEN_ENCRYPTION_KEY must be exactly 64 hex characters");
        }

        let gh_token_key = hex::decode(gh_token_key.as_bytes())
            .context("GITHUB_TOKEN_ENCRYPTION_KEY must be exactly 64 hex characters")?;

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&gh_token_key));

        Ok(Self::new(cipher))
    }

    /// Encrypts a GitHub access token using AES-256-GCM
    ///
    /// The encrypted data format is: `[12-byte nonce][encrypted data]`
    /// The nonce is randomly generated for each encryption to ensure uniqueness.
    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>> {
        // Generate a random nonce for this encryption
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        // Encrypt the token
        let encrypted = self
            .cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|error| anyhow::anyhow!("Failed to encrypt token: {error}"))?;

        // Combine nonce + ciphertext (which includes the auth tag)
        let mut result = Vec::with_capacity(nonce.len() + encrypted.len());
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&encrypted);

        Ok(result)
    }

    /// Decrypts a GitHub access token using AES-256-GCM
    ///
    /// Expects the data format: `[12-byte nonce][encrypted data]`
    pub fn decrypt(&self, encrypted: &[u8]) -> Result<SecretString> {
        if encrypted.len() < 12 {
            anyhow::bail!("Invalid encrypted token: too short");
        }

        // Extract nonce and ciphertext
        let (nonce_bytes, ciphertext) = encrypted.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt the token
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .context("Failed to decrypt token")?;

        let plaintext =
            String::from_utf8(plaintext).context("Decrypted token is not valid UTF-8")?;

        Ok(plaintext.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::{Key, KeyInit};
    use claims::{assert_err, assert_ok};
    use insta::assert_snapshot;
    use secrecy::ExposeSecret;

    fn create_test_encryption() -> GitHubTokenEncryption {
        let key = Key::<Aes256Gcm>::from_slice(b"test_master_key_32_bytes_long!!!");
        let cipher = Aes256Gcm::new(key);
        GitHubTokenEncryption { cipher }
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let encryption = create_test_encryption();
        let original_token = "ghs_test_token_123456789";

        // Encrypt the token
        let encrypted = assert_ok!(encryption.encrypt(original_token));

        // Decrypt it back
        let decrypted = assert_ok!(encryption.decrypt(&encrypted));

        assert_eq!(original_token, decrypted.expose_secret());
    }

    #[test]
    fn test_encrypt_produces_different_ciphertext() {
        let encryption = create_test_encryption();
        let token = "ghs_test_token_123456789";

        // Encrypt the same token twice
        let encrypted1 = assert_ok!(encryption.encrypt(token));
        let encrypted2 = assert_ok!(encryption.encrypt(token));

        // Should produce different ciphertext due to random nonce
        assert_ne!(encrypted1, encrypted2);

        // But both should decrypt to the same plaintext
        let decrypted1 = assert_ok!(encryption.decrypt(&encrypted1));
        let decrypted2 = assert_ok!(encryption.decrypt(&encrypted2));

        assert_eq!(decrypted1.expose_secret(), decrypted2.expose_secret());
        assert_eq!(decrypted1.expose_secret(), token);
    }

    #[test]
    fn test_invalid_encrypted_data() {
        let encryption = create_test_encryption();

        // Too short
        let err = assert_err!(encryption.decrypt(&[1, 2, 3]));
        assert_snapshot!(err, @"Invalid encrypted token: too short");

        // Invalid data
        let invalid_data = vec![0u8; 50];
        let err = assert_err!(encryption.decrypt(&invalid_data));
        assert_snapshot!(err, @"Failed to decrypt token");
    }

    #[test]
    fn test_different_keys() {
        let encryption1 = create_test_encryption();

        // Create a different encryption with a different key
        let key2 = Key::<Aes256Gcm>::from_slice(b"different_key_32_bytes_long!!!!!");
        let cipher2 = Aes256Gcm::new(key2);
        let encryption2 = GitHubTokenEncryption { cipher: cipher2 };

        let token = "ghs_test_token_123456789";

        // Encrypt with encryption1
        let encrypted = assert_ok!(encryption1.encrypt(token));

        // Try to decrypt with encryption2 (should fail)
        let err = assert_err!(encryption2.decrypt(&encrypted));
        assert_snapshot!(err, @"Failed to decrypt token");

        // But encryption1 should still work
        let decrypted = assert_ok!(encryption1.decrypt(&encrypted));
        assert_eq!(decrypted.expose_secret(), token);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use hegel::TestCase;
    use hegel::generators as gs;
    use secrecy::ExposeSecret;

    fn encryption() -> GitHubTokenEncryption {
        GitHubTokenEncryption::for_testing()
    }

    /// Round-trip: any UTF-8 token survives encrypt followed by decrypt.
    #[hegel::test(test_cases = 1000)]
    fn prop_encrypt_decrypt_roundtrip(tc: TestCase) {
        let plaintext = tc.draw(gs::text().max_size(256));
        let enc = encryption();
        let ciphertext = enc.encrypt(&plaintext).unwrap();
        let decrypted = enc.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted.expose_secret(), plaintext.as_str());
    }

    /// The random nonce makes each encryption unique, yet both decrypt back to
    /// the same plaintext.
    #[hegel::test(test_cases = 500)]
    fn prop_ciphertext_is_randomized(tc: TestCase) {
        let plaintext = tc.draw(gs::text().max_size(64));
        let enc = encryption();
        let a = enc.encrypt(&plaintext).unwrap();
        let b = enc.encrypt(&plaintext).unwrap();
        assert_ne!(a, b, "nonce reuse produced identical ciphertext");
        assert_eq!(enc.decrypt(&a).unwrap().expose_secret(), plaintext.as_str());
        assert_eq!(enc.decrypt(&b).unwrap().expose_secret(), plaintext.as_str());
    }

    /// Flipping any byte of a valid ciphertext (nonce or AEAD body) is rejected.
    #[hegel::test(test_cases = 1000)]
    fn prop_tampered_ciphertext_is_rejected(tc: TestCase) {
        let plaintext = tc.draw(gs::text().max_size(128));
        let enc = encryption();
        let mut ciphertext = enc.encrypt(&plaintext).unwrap();
        let idx = tc.draw(
            gs::integers::<usize>()
                .min_value(0)
                .max_value(ciphertext.len() - 1),
        );
        ciphertext[idx] ^= 0xff;
        assert!(
            enc.decrypt(&ciphertext).is_err(),
            "tampered ciphertext was accepted (flipped byte {idx})"
        );
    }

    /// Decryption of arbitrary bytes returns a Result and never panics.
    #[hegel::test(test_cases = 1000)]
    fn prop_decrypt_arbitrary_bytes_never_panics(tc: TestCase) {
        let bytes = tc.draw(gs::vecs(gs::integers::<u8>()).max_size(64));
        let _ = encryption().decrypt(&bytes);
    }
}
