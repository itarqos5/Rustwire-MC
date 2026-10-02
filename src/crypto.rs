//! Minecraft AES-128-CFB8, RSA PKCS#1 v1.5 key exchange, and signed SHA-1 hash.
//! The stream state must be continuous across packet boundaries.
use crate::{Error, Result};
use aes::{
    cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit},
    Aes128,
};
use rand::{rngs::OsRng, RngCore};
use rsa::{pkcs8::DecodePublicKey, Pkcs1v15Encrypt, RsaPublicKey};
use sha1::{Digest, Sha1};
use zeroize::{Zeroize, Zeroizing};
/// Stateful encryptor/decryptor. Debug intentionally omits key material.
pub struct Cipher {
    cipher: Aes128,
    feedback: [u8; 16],
}
impl std::fmt::Debug for Cipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cipher([REDACTED])")
    }
}
impl Drop for Cipher {
    fn drop(&mut self) {
        self.feedback.zeroize();
    }
}
impl Cipher {
    pub fn new(secret: &[u8; 16]) -> Self {
        Self {
            cipher: Aes128::new(GenericArray::from_slice(secret)),
            feedback: *secret,
        }
    }
    pub fn encrypt(&mut self, bytes: &mut [u8]) {
        self.apply(bytes, false);
    }
    pub fn decrypt(&mut self, bytes: &mut [u8]) {
        self.apply(bytes, true);
    }
    fn apply(&mut self, bytes: &mut [u8], decrypt: bool) {
        for byte in bytes {
            let mut block = GenericArray::clone_from_slice(&self.feedback);
            self.cipher.encrypt_block(&mut block);
            let input = *byte;
            *byte ^= block[0];
            self.feedback.copy_within(1.., 0);
            self.feedback[15] = if decrypt { input } else { *byte };
        }
    }
}
pub struct EncryptionResponse {
    pub shared_secret: [u8; 16],
    pub encrypted_secret: Vec<u8>,
    pub encrypted_verify_token: Vec<u8>,
}
impl std::fmt::Debug for EncryptionResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EncryptionResponse([REDACTED])")
    }
}
impl Drop for EncryptionResponse {
    fn drop(&mut self) {
        self.shared_secret.zeroize();
    }
}
pub fn encryption_response(
    public_key_der: &[u8],
    verify_token: &[u8],
) -> Result<EncryptionResponse> {
    if public_key_der.len() > 8192 || verify_token.len() > 1024 {
        return Err(Error::Limit("encryption request"));
    }
    let key = RsaPublicKey::from_public_key_der(public_key_der)
        .map_err(|_| Error::Invalid("RSA SubjectPublicKeyInfo"))?;
    use rsa::traits::PublicKeyParts;
    if key.n().bits() < 1024 || key.n().bits() > 4096 {
        return Err(Error::Invalid("RSA key size"));
    }
    // Reject a challenge that cannot fit PKCS#1 v1.5 before generating a secret.
    if verify_token.len() > key.size().saturating_sub(11) {
        return Err(Error::Limit("RSA verify-token plaintext"));
    }
    // The temporary also needs clearing when either encryption operation fails.
    let mut shared_secret = Zeroizing::new([0; 16]);
    OsRng
        .try_fill_bytes(&mut *shared_secret)
        .map_err(|_| Error::Io(std::io::Error::other("secure random source unavailable")))?;
    let encrypted_secret = key
        .encrypt(&mut OsRng, Pkcs1v15Encrypt, &*shared_secret)
        .map_err(|_| Error::Invalid("RSA encryption"))?;
    let encrypted_verify_token = key
        .encrypt(&mut OsRng, Pkcs1v15Encrypt, verify_token)
        .map_err(|_| Error::Invalid("RSA encryption"))?;
    Ok(EncryptionResponse {
        shared_secret: *shared_secret,
        encrypted_secret,
        encrypted_verify_token,
    })
}
/// Java BigInteger's signed hexadecimal representation of the SHA-1 digest.
/// The server ID uses Java ISO-8859-1 encoding, including `?` replacement for
/// characters outside Latin-1; the secret and public key are hashed verbatim.
pub fn server_hash(server_id: &str, secret: &[u8], public_key: &[u8]) -> String {
    let mut sha = Sha1::new();
    for ch in server_id.chars() {
        sha.update([u8::try_from(u32::from(ch)).unwrap_or(b'?')]);
    }
    sha.update(secret);
    sha.update(public_key);
    signed_hex(&sha.finalize())
}
pub fn signed_hex(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "0".into();
    }
    let negative = bytes[0] & 0x80 != 0;
    let mut magnitude = bytes.to_vec();
    if negative {
        let mut carry = true;
        for b in magnitude.iter_mut().rev() {
            *b = !*b;
            if carry {
                let (v, c) = b.overflowing_add(1);
                *b = v;
                carry = c;
            }
        }
    }
    let first = magnitude
        .iter()
        .position(|&b| b != 0)
        .unwrap_or(magnitude.len());
    if first == magnitude.len() {
        return "0".into();
    }
    let mut s = if negative {
        "-".to_owned()
    } else {
        String::new()
    };
    use std::fmt::Write;
    write!(s, "{:x}", magnitude[first]).unwrap();
    for b in &magnitude[first + 1..] {
        write!(s, "{b:02x}").unwrap();
    }
    s
}
/// Vanilla's offline-mode UUID. Offline mode does not establish identity.
pub fn offline_uuid(username: &str) -> [u8; 16] {
    use md5::Md5;
    let mut h = Md5::new();
    h.update(b"OfflinePlayer:");
    h.update(username.as_bytes());
    let mut id: [u8; 16] = h.finalize().into();
    id[6] = (id[6] & 0x0f) | 0x30;
    id[8] = (id[8] & 0x3f) | 0x80;
    id
}
